; ModuleID = 'kore_main'
source_filename = "kore_main"

%Foo = type { i64 }

@str = unnamed_addr constant [6 x i8] c"hello\00", align 8

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

define i64 @Foo_new() {
entry:
  %malloc = call ptr @malloc(i64 8)
  %field_ptr = getelementptr inbounds %Foo, ptr %malloc, i32 0, i32 0
  store i64 or (i64 lshr (i64 ptrtoint (ptr @str to i64), i64 3), i64 -2111062325329920), ptr %field_ptr, align 4
  %ptr_to_int = ptrtoint ptr %malloc to i64
  %box_ptr_qnan = or i64 %ptr_to_int, -2251799813685248
  ret i64 %box_ptr_qnan
}

define i64 @main_kore() {
entry:
  %static_call = call i64 @Foo_new()
  %f = alloca i64, align 8
  store i64 %static_call, ptr %f, align 4
  ret i64 0
}
