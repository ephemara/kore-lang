; ModuleID = 'kore_main'
source_filename = "kore_main"

@str = unnamed_addr constant [15 x i8] c"tests/hello.kr\00", align 8
@str.1 = unnamed_addr constant [9 x i8] c"String: \00", align 8
@str.2 = unnamed_addr constant [13 x i8] c"First char: \00", align 8
@str.3 = unnamed_addr constant [2 x i8] c"-\00", align 8
@str.4 = unnamed_addr constant [14 x i8] c"Starts with -\00", align 8
@str.5 = unnamed_addr constant [22 x i8] c"Does NOT start with -\00", align 8
@str.6 = unnamed_addr constant [16 x i8] c"kore_native.exe\00", align 8
@str.7 = unnamed_addr constant [3 x i8] c"-o\00", align 8
@str.8 = unnamed_addr constant [9 x i8] c"--target\00", align 8
@str.9 = unnamed_addr constant [34 x i8] c"prev is not -o or --target - PASS\00", align 8
@str.10 = unnamed_addr constant [35 x i8] c"prev matches -o or --target - FAIL\00", align 8

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
  %s = alloca i64, align 8
  store i64 or (i64 lshr (i64 ptrtoint (ptr @str to i64), i64 3), i64 -2111062325329920), ptr %s, align 4
  %s1 = load i64, ptr %s, align 4
  %call = call i64 @kore_substring(i64 %s1, i64 -2216615441596416, i64 -2216615441596415)
  %first = alloca i64, align 8
  store i64 %call, ptr %first, align 4
  %s2 = load i64, ptr %s, align 4
  %binop = call i64 @kore_add_op(i64 or (i64 lshr (i64 ptrtoint (ptr @str.1 to i64), i64 3), i64 -2111062325329920), i64 %s2)
  call void @kore_println_str(i64 %binop)
  %first3 = load i64, ptr %first, align 4
  %binop4 = call i64 @kore_add_op(i64 or (i64 lshr (i64 ptrtoint (ptr @str.2 to i64), i64 3), i64 -2111062325329920), i64 %first3)
  call void @kore_println_str(i64 %binop4)
  %first5 = load i64, ptr %first, align 4
  %call6 = call i64 @kore_str_eq(i64 %first5, i64 or (i64 lshr (i64 ptrtoint (ptr @str.3 to i64), i64 3), i64 -2111062325329920))
  %ifcond = icmp ne i64 %call6, 0
  br i1 %ifcond, label %then, label %else

then:                                             ; preds = %entry
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.4 to i64), i64 3), i64 -2111062325329920))
  br label %merge

else:                                             ; preds = %entry
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.5 to i64), i64 3), i64 -2111062325329920))
  br label %merge

merge:                                            ; preds = %else, %then
  %prev = alloca i64, align 8
  store i64 or (i64 lshr (i64 ptrtoint (ptr @str.6 to i64), i64 3), i64 -2111062325329920), ptr %prev, align 4
  %prev7 = load i64, ptr %prev, align 4
  %call8 = call i64 @kore_str_eq(i64 %prev7, i64 or (i64 lshr (i64 ptrtoint (ptr @str.7 to i64), i64 3), i64 -2111062325329920))
  %truthy = call i64 @kore_is_truthy(i64 %call8)
  %is_falsy = icmp eq i64 %truthy, 0
  %not_result = zext i1 %is_falsy to i64
  %prev9 = load i64, ptr %prev, align 4
  %call10 = call i64 @kore_str_eq(i64 %prev9, i64 or (i64 lshr (i64 ptrtoint (ptr @str.8 to i64), i64 3), i64 -2111062325329920))
  %truthy11 = call i64 @kore_is_truthy(i64 %call10)
  %is_falsy12 = icmp eq i64 %truthy11, 0
  %not_result13 = zext i1 %is_falsy12 to i64
  %l_truthy = call i64 @kore_is_truthy(i64 %not_result)
  %r_truthy = call i64 @kore_is_truthy(i64 %not_result13)
  %l_bool = icmp ne i64 %l_truthy, 0
  %r_bool = icmp ne i64 %r_truthy, 0
  %and_result = and i1 %l_bool, %r_bool
  %bool_to_i64 = zext i1 %and_result to i64
  %ifcond14 = icmp ne i64 %bool_to_i64, 0
  br i1 %ifcond14, label %then15, label %else16

then15:                                           ; preds = %merge
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.9 to i64), i64 3), i64 -2111062325329920))
  br label %merge17

else16:                                           ; preds = %merge
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.10 to i64), i64 3), i64 -2111062325329920))
  br label %merge17

merge17:                                          ; preds = %else16, %then15
  ret i64 0
}
