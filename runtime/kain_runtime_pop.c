
int64_t kain_array_pop(int64_t arr_ptr) {
    KainArray* arr = (KainArray*)arr_ptr;
    if (arr->len == 0) return 0;
    return arr->data[--arr->len];
}
