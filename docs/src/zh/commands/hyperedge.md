# 超边命令

```text
derivon hyperedge list
derivon hyperedge get <ID>
derivon hyperedge add <ID> [--tail <POINT_ID>]... --head <POINT_ID> --weight <WEIGHT>
  [--data <JSON> | --data-file <FILE>]
derivon hyperedge remove <ID> [--ignore-missing]
derivon hyperedge rename <ID> <NEW_ID>
derivon hyperedge set tails <ID> [--tail <POINT_ID>]...
derivon hyperedge set head <ID> <POINT_ID>
derivon hyperedge set weight <ID> <WEIGHT>
derivon hyperedge data get <ID> [POINTER]
derivon hyperedge data set <ID> [POINTER] (--value <JSON> | --value-file <FILE>)
derivon hyperedge data remove <ID> [POINTER] [--ignore-missing]
```

mutation 输出完整 graph。新超边追加；缺省 data 为 `{}`，显式 null 保留。CRUD 严格，rename 到自身失败；`--ignore-missing` 只用于 remove，并使未知边删除成为成功 no-op。引用点必须存在，tails 不得重复；不传 `--tail` 表示空尾。ID 不同的平行超边合法。相同值的 set 成功。

weight 使用精确十进制，必须为十分之一的整数倍且不超过 `900719925474099.1`。不经过 `f64` 或舍入。科学计数法按数学值校验，负零输出为零。

data 使用与点相同的 JSON Pointer 规则。缺失/null 下的子路径操作返回 `pointer_type_mismatch`，不会 panic 或提升类型。
