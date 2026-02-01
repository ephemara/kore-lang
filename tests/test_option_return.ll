; ModuleID = 'kore_main'
source_filename = "kore_main"

%Parser = type { i64 }

@str = unnamed_addr constant [5 x i8] c"test\00", align 8
@str.1 = unnamed_addr constant [5 x i8] c"Done\00", align 8
@str.2 = unnamed_addr constant [6 x i8] c"hello\00", align 8

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

define i64 @Parser_test_option(i64 %self, i64 %0, i64 %1) {
entry:
  %self1 = alloca i64, align 8
  store i64 %self, ptr %self1, align 4
  ret i64 or (i64 lshr (i64 ptrtoint (ptr @str.2 to i64), i64 3), i64 -2111062325329920)
}

define i64 @main_kore() {
entry:
  %malloc = call ptr @malloc(i64 8)
  %field_ptr = getelementptr inbounds %Parser, ptr %malloc, i32 0, i32 0
  store i64 or (i64 lshr (i64 ptrtoint (ptr @str to i64), i64 3), i64 -2111062325329920), ptr %field_ptr, align 4
  %ptr_to_int = ptrtoint ptr %malloc to i64
  %box_ptr_qnan = or i64 %ptr_to_int, -2251799813685248
  %p = alloca i64, align 8
  store i64 %box_ptr_qnan, ptr %p, align 4
  %p1 = load i64, ptr %p, align 4
  %result = alloca i64, align 8
  store i64 0, ptr %result, align 4
  call void @kore_println_str(i64 or (i64 lshr (i64 ptrtoint (ptr @str.1 to i64), i64 3), i64 -2111062325329920))
  ret i64 0
}
