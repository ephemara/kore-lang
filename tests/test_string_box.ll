; ModuleID = 'kore_main'
source_filename = "kore_main"

@str = unnamed_addr constant [7 x i8] c"--help\00", align 8
@str.1 = unnamed_addr constant [7 x i8] c"Match!\00", align 8
@str.2 = unnamed_addr constant [9 x i8] c"No match\00", align 8

declare ptr @malloc(i64)

declare i64 @kore_add_op(i64, i64)

declare i64 @kore_sub_op(i64, i64)

declare i64 @kore_mul_op(i64, i64)

declare i64 @kore_div_op(i64, i64)

declare i64 @kore_rem_op(i64, i64)

declare i64 @kore_eq_op(i64, i64)

declare i64 @kore_neq_op(i64, i64)

declare i64 @kore_lt_op(i64, i64)

declare i64 @kore_le_op(i64, i64)

declare i64 @kore_str_concat_boxed(i64, i64)

declare i64 @kore_gt_op(i64, i64)

declare i64 @kore_ge_op(i64, i64)

declare i64 @kore_contains(i64, i64)

declare i64 @kore_str_eq(i64, i64)

declare i64 @kore_array_get(i64, i64)

declare i64 @kore_array_len(i64, i64)

declare i64 @kore_map_get(i64, i64)

declare i64 @kore_map_set(i64, i64)

declare i64 @kore_substring(i64, i64)

declare i64 @args(i64)

declare i64 @kore_array_new(i64)

declare i64 @kore_str_len(i64)

declare i64 @kore_to_string(i64)

declare i64 @kore_is_truthy(i64)

declare void @kore_print_str(i64)

declare void @kore_println_str(i64)

define i64 @main_kore() {
entry:
  %s = alloca i64, align 8
  store i64 or (i64 lshr (i64 ptrtoint (ptr @str to i64), i64 3), i64 -2111062325329920), ptr %s, align 4
  %s1 = load i64, ptr %s, align 4
  call void @kore_println_str(i64 %s1)
  %s2 = load i64, ptr %s, align 4
  %call = call i64 @kore_str_eq(i64 %s2, i64 or (i64 lshr (i64 ptrtoint (ptr @str to i64), i64 3), i64 -2111062325329920))
  %ifcond = icmp ne i64 %call, 0
  br i1 %ifcond, label %then, label %else

then:                                             ; preds = %entry
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.1 to i64), i64 3), i64 -2111062325329920))
  br label %merge

else:                                             ; preds = %entry
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.2 to i64), i64 3), i64 -2111062325329920))
  br label %merge

merge:                                            ; preds = %else, %then
  ret i64 0
}
