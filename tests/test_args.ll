; ModuleID = 'kore_main'
source_filename = "kore_main"

@str = unnamed_addr constant [12 x i8] c"Array len: \00", align 8
@str.1 = unnamed_addr constant [5 x i8] c"Arg \00", align 8
@str.2 = unnamed_addr constant [3 x i8] c": \00", align 8
@str.3 = unnamed_addr constant [21 x i8] c"Testing substring...\00", align 8
@str.4 = unnamed_addr constant [10 x i8] c"arg[1] = \00", align 8
@str.5 = unnamed_addr constant [14 x i8] c"first char = \00", align 8
@str.6 = unnamed_addr constant [2 x i8] c"-\00", align 8
@str.7 = unnamed_addr constant [17 x i8] c"Starts with dash\00", align 8
@str.8 = unnamed_addr constant [25 x i8] c"Does NOT start with dash\00", align 8

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

declare i64 @kore_map_get(i64, i64)

declare i64 @kore_map_set(i64, i64)

declare i64 @kore_array_push(i64, i64)

declare i64 @kore_substring(i64, i64, i64)

declare i64 @args(i64)

declare i64 @kore_array_new(i64)

declare i64 @kore_str_len(i64)

declare i64 @kore_to_string(i64)

declare i64 @kore_is_truthy(i64)

declare i64 @exit(i64)

declare i64 @kore_array_len(i64)

declare void @kore_print_str(i64)

declare void @kore_println_str(i64)

define i64 @main_kore() {
entry:
  %call = call i64 @args()
  %arr = alloca i64, align 8
  store i64 %call, ptr %arr, align 4
  %arr1 = load i64, ptr %arr, align 4
  %call2 = call i64 @kore_array_len(i64 %arr1)
  %len = alloca i64, align 8
  store i64 %call2, ptr %len, align 4
  %len3 = load i64, ptr %len, align 4
  %call4 = call i64 @kore_to_string(i64 %len3)
  %binop = call i64 @kore_add_op(i64 or (i64 lshr (i64 ptrtoint (ptr @str to i64), i64 3), i64 -2111062325329920), i64 %call4)
  call void @kore_println_str(i64 %binop)
  %i = alloca i64, align 8
  store i64 -2216615441596416, ptr %i, align 4
  br label %while_cond

while_cond:                                       ; preds = %while_body, %entry
  %i5 = load i64, ptr %i, align 4
  %len6 = load i64, ptr %len, align 4
  %binop7 = call i64 @kore_lt_op(i64 %i5, i64 %len6)
  %whilecheck = icmp ne i64 %binop7, 0
  br i1 %whilecheck, label %while_body, label %while_end

while_body:                                       ; preds = %while_cond
  %item = alloca i64, align 8
  store i64 0, ptr %item, align 4
  %i8 = load i64, ptr %i, align 4
  %call9 = call i64 @kore_to_string(i64 %i8)
  %binop10 = call i64 @kore_add_op(i64 or (i64 lshr (i64 ptrtoint (ptr @str.1 to i64), i64 3), i64 -2111062325329920), i64 %call9)
  %binop11 = call i64 @kore_add_op(i64 %binop10, i64 or (i64 lshr (i64 ptrtoint (ptr @str.2 to i64), i64 3), i64 -2111062325329920))
  %item12 = load i64, ptr %item, align 4
  %binop13 = call i64 @kore_add_op(i64 %binop11, i64 %item12)
  call void @kore_println_str(i64 %binop13)
  %i14 = load i64, ptr %i, align 4
  %binop15 = call i64 @kore_add_op(i64 %i14, i64 -2216615441596415)
  store i64 %binop15, ptr %i, align 4
  br label %while_cond

while_end:                                        ; preds = %while_cond
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.3 to i64), i64 3), i64 -2111062325329920))
  %len16 = load i64, ptr %len, align 4
  %binop17 = call i64 @kore_gt_op(i64 %len16, i64 -2216615441596415)
  %ifcond = icmp ne i64 %binop17, 0
  br i1 %ifcond, label %then, label %else

then:                                             ; preds = %while_end
  %arg = alloca i64, align 8
  store i64 0, ptr %arg, align 4
  %arg18 = load i64, ptr %arg, align 4
  %binop19 = call i64 @kore_add_op(i64 or (i64 lshr (i64 ptrtoint (ptr @str.4 to i64), i64 3), i64 -2111062325329920), i64 %arg18)
  call void @kore_println_str(i64 %binop19)
  %arg20 = load i64, ptr %arg, align 4
  %call21 = call i64 @kore_substring(i64 %arg20, i64 -2216615441596416, i64 -2216615441596415)
  %first = alloca i64, align 8
  store i64 %call21, ptr %first, align 4
  %first22 = load i64, ptr %first, align 4
  %binop23 = call i64 @kore_add_op(i64 or (i64 lshr (i64 ptrtoint (ptr @str.5 to i64), i64 3), i64 -2111062325329920), i64 %first22)
  call void @kore_println_str(i64 %binop23)
  %first24 = load i64, ptr %first, align 4
  %call25 = call i64 @kore_str_eq(i64 %first24, i64 or (i64 lshr (i64 ptrtoint (ptr @str.6 to i64), i64 3), i64 -2111062325329920))
  %ifcond26 = icmp ne i64 %call25, 0
  br i1 %ifcond26, label %then27, label %else28

else:                                             ; preds = %while_end
  br label %merge

merge:                                            ; preds = %else
  ret i64 0

then27:                                           ; preds = %then
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.7 to i64), i64 3), i64 -2111062325329920))
  br label %merge29

else28:                                           ; preds = %then
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.8 to i64), i64 3), i64 -2111062325329920))
  br label %merge29

merge29:                                          ; preds = %else28, %then27
}
