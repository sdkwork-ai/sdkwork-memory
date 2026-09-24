# Runbook — sdkwork-memory 备份与恢复（中文）

## 1. 备份

```bash
bin/backup.sh create --environment production            # 配置 + 数据库 + 卷，含 sha256
bin/backup.sh list   --environment production
bin/backup.sh verify --environment production            # 校验最新集合
```

备份集位于目标机 `/opt/deploy/sdkwork-memory/backups/`。RPO：生产每日 + 每次升级前；RTO：生产 4 小时内完成恢复。

## 2. 恢复（破坏性，需 --yes）

```bash
bin/backup.sh restore --environment production --set <集合名> --yes
bin/docker-deploy.sh install --environment production     # 恢复后重新拉起
```

## 3. 演练

每季度在临时环境真实恢复一次（不是只跑 verify）。

## 4. Kubernetes 形态

### 4.1 备份 CronJob

`deployments/kubernetes/backup-cronjob.yaml` 提供集群内每日逻辑备份：

- 每日 02:17 UTC 运行一次 `pg_dump`（postgres:16-alpine），数据库连接串来自
  Secret `sdkwork-memory-database` 的 `database-url`（与 gateway / migration Job
  同一统一工作区数据库身份）。
- 备份输出到 PVC `sdkwork-memory-backups`（同 manifest 内声明），文件名
  `sdkwork-memory-<UTC时间戳>.sql.gz`，gzip -9 压缩，写完后做非空 + gzip 完整性校验。
- 保留 7 份：脚本在**成功**生成新备份后才轮转删除最旧的（失败不会缩减备份积压）。
- `concurrencyPolicy: Forbid` 防止重叠运行。生产使用前把镜像改为 digest 固定。

```bash
kubectl apply -f deployments/kubernetes/backup-cronjob.yaml
kubectl get cronjob sdkwork-memory-db-backup
# 手动触发一次并验证：
kubectl create job --from=cronjob/sdkwork-memory-db-backup sdkwork-memory-db-backup-manual
kubectl logs job/sdkwork-memory-db-backup-manual
```

### 4.2 恢复步骤（破坏性）

```bash
# 1) 隔断写入：把 gateway 副本缩到 0，避免恢复期间产生新写入。
kubectl scale deployment/sdkwork-api-memory-standalone-gateway --replicas=0

# 2) 从 PVC 解压并通过 psql 管道恢复到目标库。
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
        "command": ["/bin/sh", "-c", "gunzip -c /backups/<备份文件>.sql.gz | psql \"$SDKWORK_DATABASE_URL\""]
      }],
      "volumes": [{"name": "backups", "persistentVolumeClaim": {"claimName": "sdkwork-memory-backups"}}]
    }
  }'

# 3) 恢复应用流量。
kubectl scale deployment/sdkwork-api-memory-standalone-gateway --replicas=2
```

### 4.3 恢复后校验

```bash
# 数据库级抽查：核心表可查询，行数与备份前基线一致。
kubectl run sdkwork-memory-verify --rm -i --restart=Never \
  --image=postgres:16-alpine \
  --env="SDKWORK_DATABASE_URL=$(kubectl get secret sdkwork-memory-database -o jsonpath='{.data.database-url}' | base64 -d)" \
  -- psql "$SDKWORK_DATABASE_URL" -c 'SELECT count(*) FROM ai_space;'

# 应用级：/readyz 通过，并创建一条测试记忆确认可检索。
kubectl port-forward deploy/sdkwork-api-memory-standalone-gateway 8080:8080
curl -sf http://127.0.0.1:8080/readyz && echo READY
```

若恢复的 schema 落后于镜像版本（`/readyz` 报 canonical schema 不匹配），先执行
migration Job（见 `deployments/kubernetes/README.md`）再放开流量。

