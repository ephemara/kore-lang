// ============================================================================
// Kain Runtime Library
// ============================================================================
// This provides the runtime functions that LLVM-compiled Kain programs need.
// Compile with: clang -c kain_runtime.c -o kain_runtime.o
// Link with:    clang program.ll kain_runtime.o -o program
// ============================================================================

#include <ctype.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// =============================================================================
// Print Functions
// =============================================================================

void kain_print_i64(int64_t value) { printf("%lld\n", (long long)value); }

void kain_print_str(const char *str) { printf("%s", str); }

void kain_println_str(const char *str) { printf("%s\n", str); }

// =============================================================================
// String Operations
// =============================================================================

char *kain_str_new(const char *str) {
  size_t len = strlen(str);
  char *result = (char *)malloc(len + 1);
  strcpy(result, str);
  return result;
}

char *kain_str_concat(const char *a, const char *b) {
  size_t len_a = strlen(a);
  size_t len_b = strlen(b);
  char *result = (char *)malloc(len_a + len_b + 1);
  strcpy(result, a);
  strcat(result, b);
  return result;
}

int64_t kain_str_len(const char *str) { return (int64_t)strlen(str); }

int64_t kain_str_eq(const char *a, const char *b) {
  return strcmp(a, b) == 0 ? 1 : 0;
}

int64_t kain_ord(int64_t str_ptr) {
  const char *str = (const char *)str_ptr;
  if (!str || !*str)
    return 0;
  return (int64_t)(unsigned char)str[0];
}

// =============================================================================
// Array Operations (heap-allocated dynamic arrays)
// =============================================================================

typedef struct {
  int64_t *data;
  int64_t len;
  int64_t cap;
} KainArray;

// We store arrays as pointers cast to i64
int64_t kain_array_new() {
  KainArray *arr = (KainArray *)malloc(sizeof(KainArray));
  arr->data = NULL;
  arr->len = 0;
  arr->cap = 0;
  return (int64_t)arr;
}

void kain_array_push(int64_t arr_ptr, int64_t value) {
  KainArray *arr = (KainArray *)arr_ptr;
  if (arr->len >= arr->cap) {
    int64_t new_cap = arr->cap == 0 ? 8 : arr->cap * 2;
    arr->data = (int64_t *)realloc(arr->data, new_cap * sizeof(int64_t));
    arr->cap = new_cap;
  }
  arr->data[arr->len++] = value;
}

int64_t kain_array_get(int64_t arr_ptr, int64_t index) {
  KainArray *arr = (KainArray *)arr_ptr;
  if (index < 0 || index >= arr->len) {
    fprintf(stderr, "Array index out of bounds: %lld (len=%lld)\n",
            (long long)index, (long long)arr->len);
    exit(1);
  }
  return arr->data[index];
}

void kain_array_set(int64_t arr_ptr, int64_t index, int64_t value) {
  KainArray *arr = (KainArray *)arr_ptr;
  if (index < 0 || index >= arr->len) {
    fprintf(stderr, "Array index out of bounds: %lld (len=%lld)\n",
            (long long)index, (long long)arr->len);
    exit(1);
  }
  arr->data[index] = value;
}

int64_t kain_array_len(int64_t arr_ptr) {
  KainArray *arr = (KainArray *)arr_ptr;
  return arr->len;
}

void kain_array_free(int64_t arr_ptr) {
  KainArray *arr = (KainArray *)arr_ptr;
  if (arr->data)
    free(arr->data);
  free(arr);
}

// =============================================================================
// Helper Functions (Stdlib)
// =============================================================================

int64_t kain_contains(int64_t arr_ptr, int64_t item_ptr) {
  KainArray *arr = (KainArray *)arr_ptr;
  const char *item = (const char *)item_ptr;

  // Assume array of strings for now (as per lexer usage)
  for (int64_t i = 0; i < arr->len; i++) {
    const char *elem = (const char *)arr->data[i];
    if (strcmp(elem, item) == 0) {
      return 1;
    }
  }
  return 0;
}

int64_t kain_split(int64_t str_ptr, int64_t delim_ptr) {
  const char *str = (const char *)str_ptr;
  const char *delim = (const char *)delim_ptr;

  int64_t arr_ptr = kain_array_new();

  if (strlen(delim) == 0) {
    // Split into characters
    size_t len = strlen(str);
    for (size_t i = 0; i < len; i++) {
      char buf[2];
      buf[0] = str[i];
      buf[1] = '\0';
      kain_array_push(arr_ptr, (int64_t)kain_str_new(buf));
    }
  } else {
    // Split by delimiter
    char *str_copy = strdup(str);
    char *token = strtok(str_copy, delim);
    while (token != NULL) {
      kain_array_push(arr_ptr, (int64_t)kain_str_new(token));
      token = strtok(NULL, delim);
    }
    free(str_copy);
  }

  return arr_ptr;
}

int64_t kain_len(int64_t obj_ptr) {
  // Hack: try to guess if it's an array or string
  // In lexer.kr, len() is used for both.
  // Since we can't easily distinguish without tags, and lexer usage involves
  // both: len(tokens) -> Array len(chars) -> Array len(lexeme) -> String

  // If we assume pointers are valid...
  // KainArray is 24 bytes.
  // String is variable.

  // Let's try to interpret as KainArray first?
  // Arrays have high capacity/len usually small?
  // This is dangerous.

  // For now, defaulting to Array len because `contains` implies array
  // operations are active. Strings might break.
  // TODO: Implement tagged pointers or object headers.
  return kain_array_len(obj_ptr);
}

int64_t kain_to_int(int64_t str_ptr) {
  const char *str = (const char *)str_ptr;
  return atoll(str);
}

// Float logic is tricky because we use i64 everywhere in this bootstrap.
// We might be storing float bits in i64?
// parser.kn uses Expr::Float(float(tok.lexeme)).
// If Expr::Float stores i64, then we just cast bits?
// But let's assume standard float parsing for now, returning bits as i64?
// Or maybe we just return integer part?
// The bootstrap uses i64 for everything.
// I'll leave float for now or return 0.
int64_t kain_to_float(int64_t str_ptr) {
  return 0; // TODO: Float support
}

int64_t kain_to_string(int64_t val) {
  // lexer.kr uses str(len(tokens)) -> str(int).
  char buf[64];
  sprintf(buf, "%lld", (long long)val);
  return (int64_t)kain_str_new(buf);
}

int64_t kain_range(int64_t start, int64_t end) {
  int64_t arr_ptr = kain_array_new();
  for (int64_t i = start; i < end; i++) {
    kain_array_push(arr_ptr, i);
  }
  return arr_ptr;
}

int64_t kain_char_at(int64_t str_ptr, int64_t index) {
  const char *str = (const char *)str_ptr;
  if (index < 0 || index >= (int64_t)strlen(str))
    return 0;
  char buf[2];
  buf[0] = str[index];
  buf[1] = '\0';
  return (int64_t)kain_str_new(buf);
}

int64_t kain_substring(int64_t str_ptr, int64_t start, int64_t end) {
  const char *str = (const char *)str_ptr;
  size_t len = strlen(str);
  if (start < 0)
    start = 0;
  if (end > (int64_t)len)
    end = len;
  if (start >= end)
    return (int64_t)kain_str_new("");

  size_t sub_len = end - start;
  char *result = (char *)malloc(sub_len + 1);
  strncpy(result, str + start, sub_len);
  result[sub_len] = '\0';
  return (int64_t)result;
}

int64_t kain_slice(int64_t arr_ptr, int64_t start, int64_t end) {
  KainArray *arr = (KainArray *)arr_ptr;
  int64_t new_arr = kain_array_new();
  if (start < 0)
    start = 0;
  if (end > arr->len)
    end = arr->len;
  for (int64_t i = start; i < end; i++) {
    kain_array_push(new_arr, arr->data[i]);
  }
  return new_arr;
}

int64_t kain_append(int64_t str_ptr1, int64_t str_ptr2) {
  const char *a = (const char *)str_ptr1;
  const char *b = (const char *)str_ptr2;
  return (int64_t)kain_str_concat(a, b);
}

// =============================================================================
// Option/Box Helpers
// =============================================================================

// Option is represented as { tag: i64, value: i64 }
// tag = 0 for None, 1 for Some

typedef struct {
  int64_t tag;
  int64_t value;
} KainOption;

int64_t kain_some(int64_t value) {
  KainOption *opt = (KainOption *)malloc(sizeof(KainOption));
  opt->tag = 1;
  opt->value = value;
  return (int64_t)opt;
}

int64_t kain_none() {
  KainOption *opt = (KainOption *)malloc(sizeof(KainOption));
  opt->tag = 0;
  opt->value = 0;
  return (int64_t)opt;
}

int64_t kain_unwrap(int64_t opt_ptr) {
  KainOption *opt = (KainOption *)opt_ptr;
  if (opt->tag == 0) {
    fprintf(stderr, "PANIC: unwrap called on None\n");
    exit(1);
  }
  return opt->value;
}

// Box is just a wrapper around a value (heap-allocated)
int64_t kain_box(int64_t value) {
  int64_t *box = (int64_t *)malloc(sizeof(int64_t));
  *box = value;
  return (int64_t)box;
}

int64_t kain_unbox(int64_t box_ptr) {
  int64_t *box = (int64_t *)box_ptr;
  return *box;
}

// =============================================================================
// Memory Management
// =============================================================================

void *kain_alloc(int64_t size) { return malloc((size_t)size); }

void kain_free(void *ptr) { free(ptr); }

// =============================================================================
// Type Tags (for dynamic typing / enums)
// =============================================================================

#define kain_TAG_INT 0
#define kain_TAG_FLOAT 1
#define kain_TAG_STRING 2
#define kain_TAG_BOOL 3
#define kain_TAG_ARRAY 4
#define kain_TAG_NONE 5

typedef struct {
  int64_t tag;
  int64_t value; // or pointer cast to i64
} KainValue;

int64_t kain_value_tag(int64_t value_ptr) {
  KainValue *v = (KainValue *)value_ptr;
  return v->tag;
}

int64_t kain_value_data(int64_t value_ptr) {
  KainValue *v = (KainValue *)value_ptr;
  return v->value;
}

// =============================================================================
// File I/O
// =============================================================================

char *kain_file_read(const char *path) {
  FILE *f = fopen(path, "rb");
  if (!f)
    return NULL;

  fseek(f, 0, SEEK_END);
  long size = ftell(f);
  fseek(f, 0, SEEK_SET);

  char *content = (char *)malloc(size + 1);
  fread(content, 1, size, f);
  content[size] = '\0';
  fclose(f);

  return content;
}

int64_t kain_file_write(const char *path, const char *content) {
  FILE *f = fopen(path, "wb");
  if (!f)
    return 0;

  size_t len = strlen(content);
  fwrite(content, 1, len, f);
  fclose(f);

  return 1;
}

// =============================================================================
// Map Operations (simple key-value store using parallel arrays)
// =============================================================================

typedef struct {
  int64_t *keys;
  int64_t *values;
  int64_t len;
  int64_t cap;
} KainMap;

int64_t Map_new() {
  KainMap *map = (KainMap *)malloc(sizeof(KainMap));
  map->keys = NULL;
  map->values = NULL;
  map->len = 0;
  map->cap = 0;
  return (int64_t)map;
}

int64_t kain_contains_key(int64_t map_ptr, int64_t key_ptr) {
  KainMap *map = (KainMap *)map_ptr;
  const char *key = (const char *)key_ptr;
  for (int64_t i = 0; i < map->len; i++) {
    const char *stored_key = (const char *)map->keys[i];
    if (strcmp(stored_key, key) == 0) {
      return 1;
    }
  }
  return 0;
}

// Map lookup - returns 0 if not found
int64_t kain_map_get(int64_t map_ptr, int64_t key_ptr) {
  KainMap *map = (KainMap *)map_ptr;
  const char *key = (const char *)key_ptr;
  for (int64_t i = 0; i < map->len; i++) {
    const char *stored_key = (const char *)map->keys[i];
    if (strcmp(stored_key, key) == 0) {
      return map->values[i];
    }
  }
  return 0;
}

// Map insert/update
void kain_map_set(int64_t map_ptr, int64_t key_ptr, int64_t value) {
  KainMap *map = (KainMap *)map_ptr;
  const char *key = (const char *)key_ptr;

  // Check if key exists
  for (int64_t i = 0; i < map->len; i++) {
    const char *stored_key = (const char *)map->keys[i];
    if (strcmp(stored_key, key) == 0) {
      map->values[i] = value;
      return;
    }
  }

  // Add new key
  if (map->len >= map->cap) {
    int64_t new_cap = map->cap == 0 ? 8 : map->cap * 2;
    map->keys = (int64_t *)realloc(map->keys, new_cap * sizeof(int64_t));
    map->values = (int64_t *)realloc(map->values, new_cap * sizeof(int64_t));
    map->cap = new_cap;
  }
  map->keys[map->len] = key_ptr;
  map->values[map->len] = value;
  map->len++;
}

// =============================================================================
// String Join (for StringBuilder.build())
// =============================================================================

int64_t kain_join(int64_t arr_ptr, int64_t delim_ptr) {
  KainArray *arr = (KainArray *)arr_ptr;
  const char *delim = (const char *)delim_ptr;

  if (arr->len == 0) {
    return (int64_t)kain_str_new("");
  }

  // Calculate total length needed
  size_t delim_len = strlen(delim);
  size_t total_len = 0;
  for (int64_t i = 0; i < arr->len; i++) {
    const char *s = (const char *)arr->data[i];
    if (s)
      total_len += strlen(s);
    if (i < arr->len - 1)
      total_len += delim_len;
  }

  char *result = (char *)malloc(total_len + 1);
  result[0] = '\0';

  for (int64_t i = 0; i < arr->len; i++) {
    const char *s = (const char *)arr->data[i];
    if (s)
      strcat(result, s);
    if (i < arr->len - 1)
      strcat(result, delim);
  }

  return (int64_t)result;
}

// =============================================================================
// Variant Introspection (for enum pattern matching in interpreted code)
// =============================================================================

// Get tag name from a tagged union (simplified - returns tag number as string)
int64_t kain_variant_of(int64_t value_ptr) {
  // In our representation, enums are { tag: i64, payload: i8* }
  // We'll return the tag as a string representation
  // For now, just return tag number
  int64_t *ptr = (int64_t *)value_ptr;
  if (!ptr)
    return (int64_t)kain_str_new("None");
  int64_t tag = *ptr;
  char buf[32];
  sprintf(buf, "%lld", (long long)tag);
  return (int64_t)kain_str_new(buf);
}

// Extract field from variant by index
int64_t kain_variant_field(int64_t value_ptr, int64_t field_idx) {
  // value_ptr points to { tag, payload_ptr }
  // field 0 = tag, field 1+ = payload fields
  int64_t *ptr = (int64_t *)value_ptr;
  if (!ptr)
    return 0;

  if (field_idx == 0) {
    return ptr[0]; // tag
  } else {
    // Payload is at offset 1 (the i8* which we cast to i64)
    int64_t payload = ptr[1];
    // For simple single-field variants, just return payload
    return payload;
  }
}

// =============================================================================
// Process / System
// =============================================================================

int64_t kain_system(const char *command) { return (int64_t)system(command); }

void kain_exit(int64_t code) { exit((int)code); }

void kain_panic(const char *message) {
  fprintf(stderr, "PANIC: %s\n", message);
  exit(1);
}
