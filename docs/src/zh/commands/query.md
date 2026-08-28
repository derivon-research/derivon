# 查询命令

点集参数使用可重复的单数 flag，重复 ID 是错误：

```bash
derivon query closure --start A --start B
derivon query route --start A --target X --target Y
derivon query diagnose --start A --target X --target Y
```

省略 `--start` 表示空起始集。route 目标集至少包含一个点。集合结果按 ID 确定性排序。

closure 返回包含 `startPointIds` 和 `pointIds` 的对象。route 和 diagnose 以 `reachable` 作为判别字段：可达 route 包含点、超边、执行顺序、代价上下界、`provenOptimal` 和搜索指标；不可达 route 包含 `targetDiagnoses` 并省略不适用的求解字段。diagnose 为每个目标返回一项，可达目标的 blocking 和 cycles 为空。

不可达是成功结果。route 默认预算为 200,000 个分支节点和 10,000 毫秒。两者均可覆盖或设为零以只计算界和初始 witness。CLI 不提供 unlimited 模式或产品级硬上限。
