# 原子 Apply

```text
derivon apply --operations <FILE> [--max-operations-bytes <N>]
```

operations 不能来自 stdin 或 `-`。它是跟随 CLI 兼容版本线（1.0 前如 `0.1.x`，1.0 后按 major）的无 envelope JSON 数组。

| `op` | 必填字段 | 可选字段与缺省值 |
| --- | --- | --- |
| `point.add` | `id` | `data` = `{}` |
| `point.remove` | `id` | `cascade` = false, `ignoreMissing` = false |
| `point.rename` | `id`, `newId` | 无 |
| `point.data.set` | `id`, `value` | `pointer` = `""` |
| `point.data.remove` | `id` | `pointer` = `""`, `ignoreMissing` = false |
| `hyperedge.add` | `id`, `head`, `weight` | `tails` = `[]`, `data` = `{}` |
| `hyperedge.remove` | `id` | `ignoreMissing` = false |
| `hyperedge.rename` | `id`, `newId` | 无 |
| `hyperedge.set.tails` | `id`, `tails` | 无 |
| `hyperedge.set.head` | `id`, `head` | 无 |
| `hyperedge.set.weight` | `id`, `weight` | 无 |
| `hyperedge.data.set` | `id`, `value` | `pointer` = `""` |
| `hyperedge.data.remove` | `id` | `pointer` = `""`, `ignoreMissing` = false |

字段类型严格，未知/缺失字段使整个 apply 失败。operation issue 路径包含数组下标。操作按数组顺序执行且每个中间 graph 都合法，后续操作可引用先前创建或重命名的实体。不允许 read/query/subgraph operation。空数组成功并返回输入 graph，no-op 与严格规则和单独命令一致。

进程只修改自身 graph；全部成功后完整验证并先序列化到内存。业务或序列化失败时 stdout 为零字节。最终 transport 失败退出 74，但不能撤回 OS 已接收的字节。输入文件始终只读。
