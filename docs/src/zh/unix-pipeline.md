# Unix 管道

默认从 stdin 读取 graph：

```bash
derivon point add B < graph.json > graph-with-b.json
```

也可以使用 `--input`：

```bash
derivon --input graph.json point get A
```

存在 `--input` 时，CLI 只读取指定文件且不检查 stdin；没有 `--input` 时从 stdin 读取一个 graph。CLI 不提供原地编辑或输出文件选项，文件写入由 shell 重定向完成。`--pretty` 只影响成功 JSON 的格式。

多个 mutation 可以通过管道组合。需要全部成功或全部失败时，使用原子 `apply`：

```bash
derivon apply --operations changes.json < graph.json > updated-graph.json
```

完整 authoring 文档应先由外部工具提取 graph：

```bash
jq '.graph' workspace.json | derivon query closure --start A
```
