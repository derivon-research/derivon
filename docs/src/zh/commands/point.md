# 点命令

```text
derivon point list
derivon point get <ID>
derivon point add <ID> [--data <JSON> | --data-file <FILE>]
derivon point remove <ID> [--cascade] [--ignore-missing]
derivon point rename <ID> <NEW_ID>
derivon point data get <ID> [POINTER]
derivon point data set <ID> [POINTER] (--value <JSON> | --value-file <FILE>)
derivon point data remove <ID> [POINTER] [--ignore-missing]
```

mutation 输出完整 graph。新点追加；缺省 data 为 `{}`，显式 null 保留。rename 原子更新全部 tails/head 引用并保持点位置，rename 到自身或已有 ID 会失败。删除被引用点默认失败，`--cascade` 会删除点和所有引用超边。CRUD 默认严格，`--ignore-missing` 只用于 remove；未知点带 `--cascade --ignore-missing` 是成功 no-op。

data 命令使用 RFC 6901 JSON Pointer。省略 pointer 表示整个 data。缺失中间父路径失败；set 可创建最终对象键，数组 `-` 表示追加。根删除会省略 data，数组删除会使后续元素前移，remove 不接受 `/-`。

缺失和 null data 都合法，整体 set 可替换为任意 JSON。其下的子路径操作返回 `pointer_type_mismatch`，不会提升类型或 panic；`--ignore-missing` 不忽略类型错误。set 相同值成功；带 `--ignore-missing` 删除缺失路径是成功 no-op。
