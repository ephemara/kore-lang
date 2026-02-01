; ModuleID = 'kore_main'
source_filename = "kore_main"

@str = unnamed_addr constant [6 x i8] c"hello\00", align 8
@str.1 = unnamed_addr constant [6 x i8] c"world\00", align 8
@str.2 = unnamed_addr constant [24 x i8] c"Testing str_eq results:\00", align 8
@str.3 = unnamed_addr constant [20 x i8] c"eq1 (should be 1): \00", align 8
@str.4 = unnamed_addr constant [20 x i8] c"eq2 (should be 0): \00", align 8
@str.5 = unnamed_addr constant [21 x i8] c"Testing && operator:\00", align 8
@str.6 = unnamed_addr constant [20 x i8] c"AND operator works!\00", align 8
@str.7 = unnamed_addr constant [20 x i8] c"AND operator BROKEN\00", align 8
@str.8 = unnamed_addr constant [20 x i8] c"Testing ! operator:\00", align 8
@str.9 = unnamed_addr constant [20 x i8] c"NOT operator works!\00", align 8
@str.10 = unnamed_addr constant [20 x i8] c"NOT operator BROKEN\00", align 8

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
  %a = alloca i64, align 8
  store i64 or (i64 lshr (i64 ptrtoint (ptr @str to i64), i64 3), i64 -2111062325329920), ptr %a, align 4
  %b = alloca i64, align 8
  store i64 or (i64 lshr (i64 ptrtoint (ptr @str.1 to i64), i64 3), i64 -2111062325329920), ptr %b, align 4
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.2 to i64), i64 3), i64 -2111062325329920))
  %a1 = load i64, ptr %a, align 4
  %call = call i64 @kore_str_eq(i64 %a1, i64 or (i64 lshr (i64 ptrtoint (ptr @str to i64), i64 3), i64 -2111062325329920))
  %eq1 = alloca i64, align 8
  store i64 %call, ptr %eq1, align 4
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.3 to i64), i64 3), i64 -2111062325329920))
  %eq12 = load i64, ptr %eq1, align 4
  %call3 = call i64 @kore_to_string(i64 %eq12)
  call void @kore_println_str(i64 %call3)
  %a4 = load i64, ptr %a, align 4
  %call5 = call i64 @kore_str_eq(i64 %a4, i64 or (i64 lshr (i64 ptrtoint (ptr @str.1 to i64), i64 3), i64 -2111062325329920))
  %eq2 = alloca i64, align 8
  store i64 %call5, ptr %eq2, align 4
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.4 to i64), i64 3), i64 -2111062325329920))
  %eq26 = load i64, ptr %eq2, align 4
  %call7 = call i64 @kore_to_string(i64 %eq26)
  call void @kore_println_str(i64 %call7)
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.5 to i64), i64 3), i64 -2111062325329920))
  %a8 = load i64, ptr %a, align 4
  %call9 = call i64 @kore_str_eq(i64 %a8, i64 or (i64 lshr (i64 ptrtoint (ptr @str to i64), i64 3), i64 -2111062325329920))
  %a10 = load i64, ptr %a, align 4
  %call11 = call i64 @kore_str_eq(i64 %a10, i64 or (i64 lshr (i64 ptrtoint (ptr @str.1 to i64), i64 3), i64 -2111062325329920))
  %truthy = call i64 @kore_is_truthy(i64 %call11)
  %is_falsy = icmp eq i64 %truthy, 0
  %not_result = zext i1 %is_falsy to i64
  %l_truthy = call i64 @kore_is_truthy(i64 %call9)
  %r_truthy = call i64 @kore_is_truthy(i64 %not_result)
  %l_bool = icmp ne i64 %l_truthy, 0
  %r_bool = icmp ne i64 %r_truthy, 0
  %and_result = and i1 %l_bool, %r_bool
  %bool_to_i64 = zext i1 %and_result to i64
  %ifcond = icmp ne i64 %bool_to_i64, 0
  br i1 %ifcond, label %then, label %else

then:                                             ; preds = %entry
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.6 to i64), i64 3), i64 -2111062325329920))
  br label %merge

else:                                             ; preds = %entry
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.7 to i64), i64 3), i64 -2111062325329920))
  br label %merge

merge:                                            ; preds = %else, %then
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.8 to i64), i64 3), i64 -2111062325329920))
  %a12 = load i64, ptr %a, align 4
  %call13 = call i64 @kore_str_eq(i64 %a12, i64 or (i64 lshr (i64 ptrtoint (ptr @str.1 to i64), i64 3), i64 -2111062325329920))
  %truthy14 = call i64 @kore_is_truthy(i64 %call13)
  %is_falsy15 = icmp eq i64 %truthy14, 0
  %not_result16 = zext i1 %is_falsy15 to i64
  %ifcond17 = icmp ne i64 %not_result16, 0
  br i1 %ifcond17, label %then18, label %else19

then18:                                           ; preds = %merge
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.9 to i64), i64 3), i64 -2111062325329920))
  br label %merge20

else19:                                           ; preds = %merge
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.10 to i64), i64 3), i64 -2111062325329920))
  br label %merge20

merge20:                                          ; preds = %else19, %then18
  ret i64 0
}
