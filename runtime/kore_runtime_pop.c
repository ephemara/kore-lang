
int64_t kore_array_pop(int64_t arr_ptr) {
    KoreArray* arr = (KoreArray*)arr_ptr;
    if (arr->len == 0) return 0;
    return arr->data[--arr->len];
}
