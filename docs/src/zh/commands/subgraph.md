# 子图命令

所有 subgraph 命令返回统一 envelope：

```json
{"graph":{"points":[],"hyperedges":[]},"selection":{}}
```

需要继续管道处理 graph 时显式使用 `jq '.graph'`。

## Induced

`subgraph induced [--point <POINT_ID>]...` 保留指定点，以及 head 和全部 tails 都位于指定点集内的所有超边。孤立选中点和符合条件的空尾超边会保留；没有 point flag 时返回空图。

## Reachable

`subgraph reachable --start A` 先在完整输入 graph 上计算闭包，再返回该闭包点集的 induced subgraph。它保留所有可执行备选超边，而非只保留一条 route。

## Route

`subgraph route --start A --target X` 保留当前最佳 route 的超边、所有相关端点，以及没有连接到选中边的请求 start/target 点。`selection` 是完整 route result，包含代价界和 `provenOptimal`，不会隐藏近似结果。不可达时 `graph` 为 null。

所有投影保留 schema presence、原数组顺序、ID、权重和 opaque data。点集 flag 可重复，重复 ID 是错误。
