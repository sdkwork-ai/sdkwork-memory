# Runbook — sdkwork-memory backup & restore (EN)

## 1. Backup

```bash
bin/backup.sh create --environment production            # config + database + volumes, sha256 checksummed
bin/backup.sh list   --environment production
bin/backup.sh verify --environment production            # verify the latest set
```

Backup sets live on the target host under `/opt/deploy/sdkwork-memory/backups/`.
RPO: daily in production plus before every upgrade; RTO: production restore
completes within 4 hours.

## 2. Restore (destructive, requires --yes)

```bash
bin/backup.sh restore --environment production --set <set-name> --yes
bin/docker-deploy.sh install --environment production     # bring the stack back up after restore
```

## 3. Drill

Once per quarter, perform a real restore into a scratch environment (not just
`verify`).

## 4. Kubernetes form

### 4.1 Backup CronJob

`deployments/kubernetes/backup-cronjob.yaml` provides daily in-cluster logical
backups:

- Runs `pg_dump` (postgres:16-alpine) daily at 02:17 UTC. The connection string
  comes from Secret `sdkwork-memory-database`, key `database-url` — the same
  unified workspace database identity as the gateway and the migration Job.
- Dumps land on the `sdkwork-memory-backups` PVC (declared in the same
  manifest) as `sdkwork-memory-<UTC-timestamp>.sql.gz`, gzip -9 compressed,
  then checked for non-zero size and gzip integrity.
- Retention: 7 newest dumps. Rotation deletes old dumps only AFTER a
  successful new dump, so a failed run never shrinks the backlog.
- `concurrencyPolicy: Forbid` prevents overlapping runs. Pin the image by
  digest before production use.

```bash
kubectl apply -f deployments/kubernetes/backup-cronjob.yaml
kubectl get cronjob sdkwork-memory-db-backup
# Trigger and verify one run manually:
kubectl create job --from=cronjob/sdkwork-memory-db-backup sdkwork-memory-db-backup-manual
kubectl logs job/sdkwork-memory-db-backup-manual
```

### 4.2 Restore (destructive)

```bash
# 1) Stop writes: scale the gateway to zero.
kubectl scale deployment/sdkwork-api-memory-standalone-gateway --replicas=0

# 2) Stream the dump from the PVC through psql into the target database.
kubectl run sdkwork-memory-restore --rm -i --restart=Never \
  --image=postgres:16-alpine \
  --overrides='{
    "spec": {
      "containers": [{
        "name": "sdkwork-memory-restore",
        "image": "postgres:16-alpine",
        "stdin": true,
        "stdinOnce": true,
        "env": [{
          "name": "SDKWORK_DATABASE_URL",
          "valueFrom": {"secretKeyRef": {"name": "sdkwork-memory-database", "key": "database-url"}}
        }],
        "volumeMounts": [{"name": "backups", "mountPath": "/backups"}],
        "command": ["/bin/sh", "-c", "gunzip -c /backups/<dump-file>.sql.gz | psql \"$SDKWORK_DATABASE_URL\""]
      }],
      "volumes": [{"name": "backups", "persistentVolumeClaim": {"claimName": "sdkwork-memory-backups"}}]
    }
  }'

# 3) Bring traffic back.
kubectl scale deployment/sdkwork-api-memory-standalone-gateway --replicas=2
```

### 4.3 Post-restore verification

```bash
# Database-level spot check: core tables queryable, row counts match the
# pre-incident baseline.
kubectl run sdkwork-memory-verify --rm -i --restart=Never \
  --image=postgres:16-alpine \
  --env="SDKWORK_DATABASE_URL=$(kubectl get secret sdkwork-memory-database -o jsonpath='{.data.database-url}' | base64 -d)" \
  -- psql "$SDKWORK_DATABASE_URL" -c 'SELECT count(*) FROM ai_space;'

# Application-level: /readyz passes; create a probe memory and retrieve it.
kubectl port-forward deploy/sdkwork-api-memory-standalone-gateway 8080:8080
curl -sf http://127.0.0.1:8080/readyz && echo READY
```

If the restored schema lags the image (a `/readyz` canonical-schema mismatch),
run the migration Job first (see `deployments/kubernetes/README.md`) before
restoring traffic.

