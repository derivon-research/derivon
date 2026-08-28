# 路径语义

给定起始点集 `S` 和目标点集 `T`，路径是一个超边集合 `R`，使 `S` 在 `R` 下的闭包包含 `T` 中所有目标。

```text
cost(R) = sum(weight(h)) for h in R
```

一条被多个目标或分支复用的超边只计费一次。这是 B-超图上的最小集合代价，不是普通最短路。

最小集合代价是 NP-hard 问题。搜索默认预算为 200,000 个节点和 10,000 毫秒，允许覆盖或设为零，但没有 unlimited 模式。

完成最优性证明时返回 `provenOptimal: true`。预算耗尽时仍返回当前最佳可执行推导、认证下界、上界以及 `provenOptimal: false`。此时 `cost` 只是当前最佳代价，不得解释为精确最小值。

已经位于起始集中的目标不需要超边，代价为零。任一目标不可达时返回 `reachable: false`，而不是把 graph 判为格式错误。

## 确定性

可达性、精确最优代价和认证 bounds 不受 point/hyperedge 数组重排影响，但等价重排 graph 的同代价 witness 不要求 canonical。相同输入和 node budget 且未先触发 wall-time 时 witness 确定。

集合 ID 按 ASCII 排序，executable order 不排序。blocking ID 与 cycle 内 ID 排序后，cycles 再按 ID 序列排序。`millis` 不可复现；wall-time 先触发时，不同机器的 best-known witness 和计数可以不同，但返回的 bounds 和 witness 始终有效。
