# SDKWork Memory Runbooks

Operational runbooks per `../sdkwork-specs/DOCUMENTATION_SPEC.md` section 7.

| Runbook | Topic |
| --- | --- |
| [RUNBOOK-token-key-rotation.md](RUNBOOK-token-key-rotation.md) | Token and key rotation |
| [RUNBOOK-tenant-isolation-incident.md](RUNBOOK-tenant-isolation-incident.md) | Tenant isolation incident response |
| [RUNBOOK-migration-rollback.md](RUNBOOK-migration-rollback.md) | Migration rollback |
| [RUNBOOK-provider-outage.md](RUNBOOK-provider-outage.md) | Provider outage |
| [RUNBOOK-rate-limit-quota.md](RUNBOOK-rate-limit-quota.md) | Rate limit and quota incidents |
| [RUNBOOK-outbox-backlog.md](RUNBOOK-outbox-backlog.md) | Outbox backlog and delivery failures |
| [RUNBOOK-audit-log-investigation.md](RUNBOOK-audit-log-investigation.md) | Audit log investigation |
| [RUNBOOK-memory-pc-operations.md](RUNBOOK-memory-pc-operations.md) | Console/Admin runtime, smoke test, incident triage, and rollback |

Deployment rollout: [../deployments/runbooks/rollout.md](../deployments/runbooks/rollout.md)


<!-- scaffold-module-runbooks:index -->
## Docker 运维四件套（bin/ 标准，OPERATIONS_SPEC.md §7）

| Runbook | 内容 |
| --- | --- |
| [deploy.md](deploy.md) / [deploy.en.md](deploy.en.md) | 安装 / 升级 / 回滚 / 下线（bin/docker-deploy.sh + bin/docker-image.sh） |
| [troubleshooting.md](troubleshooting.md) / [troubleshooting.en.md](troubleshooting.en.md) | 症状 → doctor 检查 → 处置 |
| [backup-restore.md](backup-restore.md) / [backup-restore.en.md](backup-restore.en.md) | 备份 / 校验 / 恢复 / 演练（bin/backup.sh） |
| [log-reference.md](log-reference.md) / [log-reference.en.md](log-reference.en.md) | 健康日志特征与失败签名（bin/docker-deploy.sh logs） |
