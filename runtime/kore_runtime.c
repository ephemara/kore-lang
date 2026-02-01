// ============================================================================
// KORE Runtime Library
// ============================================================================
// This provides the runtime functions that LLVM-compiled Kore programs need.
// Compile with: clang -c kore_runtime.c -o kore_runtime.o
// Link with:    clang program.ll kore_runtime.o -o program
// ============================================================================

#define _CRT_SECURE_NO_WARNINGS
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <ctype.h>
#include <malloc.h>

// =============================================================================
// NaN-Boxing Type System
// =============================================================================
// 
// We use NaN-boxing to encode multiple types in a single 64-bit value.
// IEEE 754 doubles have many "quiet NaN" bit patterns we can hijack.
//
// Bit layout:
//   Float:  Any value < NANBOX_QNAN is a valid IEEE 754 double (unboxed)
//   Tagged: [0xFFF8 prefix (16 bits)][tag (3 bits)][payload (45 bits)]
//
// Type tags (3 bits):
//   0 = Pointer (heap object, 45-bit address space = 32TB)
//   1 = Integer (signed 45-bit, range: ±17.5 trillion)
//   2 = Boolean (payload = 0 or 1)
//   3 = Null/Unit
//   4-7 = Reserved for future types
//
// References: V8, LuaJIT, SpiderMonkey, JavaScriptCore, Koka
// =============================================================================

// Quiet NaN prefix - any value >= this is a tagged value, not a double
#define NANBOX_QNAN     0xFFF8000000000000ULL

// Type tag shifts and masks
#define NANBOX_TAG_SHIFT    45
#define NANBOX_PAYLOAD_MASK 0x00001FFFFFFFFFFFULL  // 45 bits

// Type tags (stored in bits 45-47)
#define KORE_TAG_PTR    0ULL
#define KORE_TAG_INT    1ULL
#define KORE_TAG_BOOL   2ULL
#define KORE_TAG_NULL   3ULL
#define KORE_TAG_STR    4ULL  // String pointers (for quick type checks)

// === UNIFIED MEMORY MODEL MACROS ===
// Standardized macros for NaN-boxing to ensure consistency across runtime and codegen
#define KORE_BOX_PTR(ptr)    (NANBOX_QNAN | (((uint64_t)(ptr)) >> 3))
#define KORE_UNBOX_PTR(val)  ((void*)(((val) & NANBOX_PAYLOAD_MASK) << 3))
// Box string: QNAN | (TAG << 45) | (ptr >> 3)
#define KORE_BOX_STR(ptr)    ((NANBOX_QNAN | (KORE_TAG_STR << NANBOX_TAG_SHIFT)) | (((uint64_t)(ptr)) >> 3))
// Unbox string: (val & PAYLOAD_MASK) << 3
#define KORE_UNBOX_STR(val)  ((const char*)(((val) & NANBOX_PAYLOAD_MASK) << 3))
// Check string: (val & PREFIX_MASK) == STR_PREFIX
#define KORE_IS_STR(val)     (((val) & (NANBOX_QNAN | (7ULL << NANBOX_TAG_SHIFT))) == (NANBOX_QNAN | (KORE_TAG_STR << NANBOX_TAG_SHIFT)))

void kore_print_stack_trace(void);

// Sentinel values
#define KORE_NULL       (NANBOX_QNAN | (KORE_TAG_NULL << NANBOX_TAG_SHIFT))
#define KORE_TRUE       (NANBOX_QNAN | (KORE_TAG_BOOL << NANBOX_TAG_SHIFT) | 1)
#define KORE_FALSE      (NANBOX_QNAN | (KORE_TAG_BOOL << NANBOX_TAG_SHIFT) | 0)

// === Type Checking ===

static inline int kore_is_double(uint64_t v) {
    return v < NANBOX_QNAN;
}

static inline int kore_is_tagged(uint64_t v) {
    return v >= NANBOX_QNAN;
}

static inline uint64_t kore_get_tag(uint64_t v) {
    if (v == 0) return KORE_TAG_NULL; // Treat 0 as Null
    if (v < NANBOX_QNAN) {
        // TRANSITION HACK: If the value is small (e.g. < 1 million), 
        // it's almost certainly a raw integer from the V1 compiler.
        if (v < 0x0010000000000000ULL) return KORE_TAG_INT; 
        return (uint64_t)-1;  // -1 = double
    }
    return (v >> NANBOX_TAG_SHIFT) & 0x7;
}

static inline int kore_is_ptr(uint64_t v) {
    if (v == 0) return 0;
    if (v < NANBOX_QNAN) return v > 0x10000; // V1 Pointer
    return ((v >> NANBOX_TAG_SHIFT) & 0x7) == KORE_TAG_PTR && (v & NANBOX_QNAN) == NANBOX_QNAN;
}

static inline int kore_is_string(uint64_t v) {
    if (v == 0) return 0;
    if (v < NANBOX_QNAN) return v > 0x10000; // V1 String
    return ((v >> NANBOX_TAG_SHIFT) & 0x7) == KORE_TAG_STR && (v & NANBOX_QNAN) == NANBOX_QNAN;
}

static inline int kore_is_int(uint64_t v) {
    if (v < NANBOX_QNAN) return v < 0x0010000000000000ULL;
    return ((v >> NANBOX_TAG_SHIFT) & 0x7) == KORE_TAG_INT && (v & NANBOX_QNAN) == NANBOX_QNAN;
}

static inline int kore_is_bool(uint64_t v) {
    return ((v >> NANBOX_TAG_SHIFT) & 0x7) == KORE_TAG_BOOL;
}

static inline int kore_is_null(uint64_t v) {
    if (v == 0) return 1;
    if (v < NANBOX_QNAN) return 0;
    return ((v >> NANBOX_TAG_SHIFT) & 0x7) == KORE_TAG_NULL;
}

// === Boxing (Kore -> NaN-box) ===

static inline uint64_t kore_box_double(double d) {
    uint64_t bits;
    memcpy(&bits, &d, sizeof(double));
    return bits;
}

static inline uint64_t kore_box_ptr(void* p) {
    if (p == NULL) return 0;
    return KORE_BOX_PTR(p);
}

static inline uint64_t kore_box_string(const char* s) {
    return KORE_BOX_STR(s);
}

static inline uint64_t kore_box_int(int64_t n) {
    // Apply NaN-boxing: QNAN | (TAG_INT << 45) | (value & PAYLOAD_MASK)
    return NANBOX_QNAN | (KORE_TAG_INT << NANBOX_TAG_SHIFT) | ((uint64_t)n & NANBOX_PAYLOAD_MASK);
}

static inline uint64_t kore_box_bool(int b) {
    return b ? KORE_TRUE : KORE_FALSE;
}

static inline uint64_t kore_box_null(void) {
    return KORE_NULL;
}

// === Unboxing (NaN-box -> Kore) ===

static inline double kore_unbox_double(uint64_t v) {
    double d;
    memcpy(&d, &v, sizeof(double));
    return d;
}

static inline void* kore_unbox_ptr(uint64_t v) {
    if (v == 0) return NULL;
    return KORE_UNBOX_PTR(v);
}

static inline const char* kore_unbox_string(uint64_t v) {
    if (KORE_IS_STR(v)) {
        return KORE_UNBOX_STR(v);
    }
    // Fallback for raw pointers (transition period)
    if (v < NANBOX_QNAN && v > 0x10000) return (const char*)v;
    return NULL;
}

// Robust unboxing that handles both tagged and raw pointers
void* kore_unbox_any_ptr(int64_t val) {
    uint64_t v = (uint64_t)val;
    if (v == 0) return NULL;
    
    // Check if it's explicitly tagged as a pointer or string
    if (v >= NANBOX_QNAN) {
       uint64_t tag = (v >> NANBOX_TAG_SHIFT) & 0x7;
       if (tag == KORE_TAG_PTR || tag == KORE_TAG_STR) {
           return KORE_UNBOX_PTR(v);
       }
       // If it's tagged as something else (Int/Bool/Null), it's NOT a pointer!
       return NULL;
    }
    
    // Raw pointer fallback (v < NANBOX_QNAN)
    return (void*)v;
}

// Forward declaration for kore_to_string (used in kore_add_op before definition)
int64_t kore_to_string(int64_t val);

static inline int64_t kore_unbox_int(uint64_t v) {
    // Sign-extend from 45 bits
    int64_t raw = (int64_t)(v & NANBOX_PAYLOAD_MASK);
    // If bit 44 is set, extend the sign
    if (raw & (1ULL << 44)) {
        raw |= 0xFFFFE00000000000LL;  // Set upper bits for negative
    }
    return raw;
}

static inline int kore_unbox_bool(uint64_t v) {
    return (v & NANBOX_PAYLOAD_MASK) != 0;
}

// Check if a value is "truthy" for condition checks
// Handles both NaN-boxed and legacy raw values
int64_t kore_is_truthy(int64_t val) {
    uint64_t v = (uint64_t)val;
    
    // CRITICAL FIX: Handle raw booleans (1/0) from comparison ops FIRST
    // Comparison operators return raw 1/0, not tagged booleans
    if (v == 1) return 1;   // Raw true
    if (v == 0) return 0;   // Raw false
    
    if (kore_is_bool(v)) {
        return (int64_t)kore_unbox_bool(v);
    } else if (kore_is_int(v)) {
        return (int64_t)(kore_unbox_int(v) != 0);
    } else if (kore_is_null(v)) {
        return 0;
    } else if (kore_is_string(v) || kore_get_tag(v) == KORE_TAG_PTR) {
        // Non-null pointers/strings are truthy. 
        return (int64_t)(v != 0);
    } else {
        // Legacy/Raw:
        return (int64_t)(val != 0);
    }
}

// =============================================================================
// CLI Arguments
// =============================================================================

static int g_argc = 0;
static char** g_argv = NULL;

void kore_set_args(int argc, char** argv) {
    g_argc = argc;
    g_argv = argv;
}

// args() implementation moved to end of file to resolve implicit declaration errors
// BUT we need it here for linking if main.kr calls it.
// To avoid implicit declaration errors in kore_runtime.c itself (if any), we should put it after helpers.
// However, since we are adding wrappers for linking, we can put them at the end.

// ... existing code ...

// =============================================================================
// Print Functions
// =============================================================================

int64_t kore_print_i64(int64_t value) {
    // Auto-unbox NaN-boxed integers
    int64_t to_print;
    if (kore_is_int((uint64_t)value)) {
        to_print = kore_unbox_int((uint64_t)value);
    } else {
        to_print = value;  // Already raw or unknown
    }
    printf("%lld\n", (long long)to_print);
    return 0;
}

int64_t kore_print_str(int64_t val) {
    const char* str = (const char*)kore_unbox_any_ptr(val);
    printf("%s", str ? str : "(null)");
    return 0;
}

int64_t kore_println_str(int64_t val) {
    kore_print_str(val);
    printf("\n");
    fflush(stdout);
    return 0;
}

// =============================================================================
// Memory Management (Paged Arena Allocator)
// =============================================================================

typedef struct ArenaPage {
    struct ArenaPage* next;
    char* data;
    size_t used;
    size_t capacity;
} ArenaPage;

static ArenaPage* head_page = NULL;
static ArenaPage* current_page = NULL;

#define PAGE_SIZE (1 * 1024 * 1024) // 1MB pages

static size_t total_allocated = 0;
#define MAX_MEMORY_USAGE (16ULL * 1024 * 1024 * 1024) // 16GB Limit

void* arena_alloc(size_t size) {
    void* p = malloc(size);
    if (!p) {
        fprintf(stderr, "FATAL: Out of memory in arena_alloc (requested %zu bytes)\n", size);
        exit(1);
    }
    return p;
}

// Global allocator wrapper - Returns RAW pointer for bootstrap compatibility
void* kore_alloc(int64_t size) {
    return arena_alloc((size_t)size);
}

void kore_free(void* ptr) {
    // No-op: Arena memory is freed when the process exits
}

char* kore_str_new(const char* str) {
    if (!str) return NULL;
    size_t len = strlen(str);
    
    char* result = _strdup(str);
    
    if (!result) {
        fprintf(stderr, "FATAL: Out of memory in kore_str_new (requested %zu bytes)\n", len + 1);
        exit(1);
    }
    return result;
}

char* kore_str_concat(const char* a, const char* b) {
    if (!a) a = "";
    if (!b) b = "";
    
    size_t len_a = strlen(a);
    size_t len_b = strlen(b);
    char* result = (char*)kore_alloc(len_a + len_b + 1);
    memcpy(result, a, len_a);
    memcpy(result + len_a, b, len_b);
    result[len_a + len_b] = '\0';
    return result;
}

// NaN-boxing aware string concat - accepts boxed values, returns boxed value
int64_t kore_str_concat_boxed(int64_t a_val, int64_t b_val) {
    const char* a = (const char*)kore_unbox_any_ptr(a_val);
    const char* b = (const char*)kore_unbox_any_ptr(b_val);
    
    char* result = kore_str_concat(a, b);
    return (int64_t)kore_box_string(result);
}



int64_t kore_str_starts_with(int64_t str_val, int64_t prefix_val) {
    const char* str = kore_is_string((uint64_t)str_val) ? kore_unbox_string((uint64_t)str_val) : (const char*)str_val;
    const char* prefix = kore_is_string((uint64_t)prefix_val) ? kore_unbox_string((uint64_t)prefix_val) : (const char*)prefix_val;
    if (!str || !prefix) return (int64_t)kore_box_bool(0);
    size_t len_str = strlen(str);
    size_t len_prefix = strlen(prefix);
    if (len_prefix > len_str) return (int64_t)kore_box_bool(0);
    return (int64_t)kore_box_bool(strncmp(str, prefix, len_prefix) == 0);
}

int64_t kore_str_replace(int64_t str_val, int64_t old_val, int64_t new_val) {
    const char* str = kore_is_string((uint64_t)str_val) ? kore_unbox_string((uint64_t)str_val) : (const char*)str_val;
    const char* old_sub = kore_is_string((uint64_t)old_val) ? kore_unbox_string((uint64_t)old_val) : (const char*)old_val;
    const char* new_sub = kore_is_string((uint64_t)new_val) ? kore_unbox_string((uint64_t)new_val) : (const char*)new_val;

    if (!str || !old_sub || !new_sub) return (int64_t)kore_box_string(kore_str_new(""));
    size_t str_len = strlen(str);
    size_t old_len = strlen(old_sub);
    size_t new_len = strlen(new_sub);
    if (old_len == 0) return (int64_t)kore_box_string(kore_str_new(str));
    
    int count = 0;
    const char* tmp = str;
    while((tmp = strstr(tmp, old_sub))) {
        count++;
        tmp += old_len;
    }
    
    size_t result_len = str_len + count * (new_len - old_len);
    char* result = (char*)arena_alloc(result_len + 1);
    char* dst = result;
    const char* src = str;
    while(1) {
        const char* p = strstr(src, old_sub);
        if (!p) {
            strcpy(dst, src);
            break;
        }
        size_t segment_len = p - src;
        memcpy(dst, src, segment_len);
        dst += segment_len;
        memcpy(dst, new_sub, new_len);
        dst += new_len;
        src = p + old_len;
    }
    return (int64_t)kore_box_string(result);
}

int64_t kore_str_len(int64_t str_val) {
    if (str_val == 0) return (int64_t)kore_box_int(0);
    
    // Auto-unbox NaN-boxed string
    const char* str;
    if (kore_is_string((uint64_t)str_val)) {
        str = kore_unbox_string((uint64_t)str_val);
    } else {
        str = (const char*)str_val;
    }
    
    return (int64_t)kore_box_int(strlen(str));
}

static int str_eq_count = 0;
int64_t kore_str_eq(int64_t a_val, int64_t b_val) {
    uint64_t a = (uint64_t)a_val;
    uint64_t b = (uint64_t)b_val;
    
    
    // Fast path: bitwise equality
    if (a_val == b_val) {
        return KORE_TRUE;
    }
    
    // NaN-boxing: Check if both are tagged integers
    if (kore_is_int(a) && kore_is_int(b)) {
        return kore_unbox_int(a) == kore_unbox_int(b) ? KORE_TRUE : KORE_FALSE;
    }
    
    // NaN-boxing: Check if both are booleans
    if (kore_is_bool(a) && kore_is_bool(b)) {
        return kore_unbox_bool(a) == kore_unbox_bool(b) ? KORE_TRUE : KORE_FALSE;
    }
    
    // NaN-boxing: Check if both are null
    if (kore_is_null(a) && kore_is_null(b)) {
        return KORE_TRUE;
    }
    
    // NaN-boxing: Check if both are strings (proper tagged)
    if (kore_is_string(a) && kore_is_string(b)) {
        const char* sa = kore_unbox_string(a);
        const char* sb = kore_unbox_string(b);
        if (!sa || !sb) return sa == sb ? KORE_TRUE : KORE_FALSE;
        return strcmp(sa, sb) == 0 ? KORE_TRUE : KORE_FALSE;
    }
    
    // V1 COMPATIBILITY: Try to extract string pointers from V1-style boxing
    // V1 uses: (ptr >> 3) | STRING_TAG
    // We can try to undo this if the value looks like it might be V1-boxed
    const char* str_a = NULL;
    const char* str_b = NULL;
    
    // Check if 'a' looks like a V1-boxed string or raw pointer
    // V1 COMPATIBILITY: Only try if not already identified as another type
    if (kore_is_int(a) || kore_is_bool(a) || kore_is_null(a)) {
        // Skip
    } else if (a >= NANBOX_QNAN) {
        uint64_t payload = a & NANBOX_PAYLOAD_MASK;
        str_a = (const char*)(payload << 3);
    } else if (a > 0x10000 && a < NANBOX_QNAN) {
        str_a = (const char*)a;
    }
    
    if (kore_is_int(b) || kore_is_bool(b) || kore_is_null(b)) {
        // Skip
    } else if (b >= NANBOX_QNAN) {
        uint64_t payload = b & NANBOX_PAYLOAD_MASK;
        str_b = (const char*)(payload << 3);
    } else if (b > 0x10000 && b < NANBOX_QNAN) {
        str_b = (const char*)b;
    }
    
    
    // If we got valid-looking string pointers, compare them
    if (str_a && str_b) {
        // Sanity check: str_a and str_b should be valid readable pointers
        // Just do strcmp - if they're garbage pointers, this will crash
        // which is better than silently returning wrong results
        int result = strcmp(str_a, str_b) == 0 ? 1 : 0;
        return result;
    }
    
    // Last resort: if one is null and the other isn't, they're not equal
    return 0;
}



int64_t kore_add_op(int64_t a_val, int64_t b_val) {
    uint64_t a = (uint64_t)a_val;
    uint64_t b = (uint64_t)b_val;
    
    // NaN-boxing: Both are tagged integers -> integer add
    if (kore_is_int(a) && kore_is_int(b)) {
        int64_t result = kore_unbox_int(a) + kore_unbox_int(b);
        return (int64_t)kore_box_int(result);
    }
    
    // NaN-boxing: Both are doubles -> double add
    if (kore_is_double(a) && kore_is_double(b)) {
        double result = kore_unbox_double(a) + kore_unbox_double(b);
        return (int64_t)kore_box_double(result);
    }
    
    // NaN-boxing: Both are tagged strings -> string concat
    if (kore_is_string(a) && kore_is_string(b)) {
        const char* sa = kore_unbox_string(a);
        const char* sb = kore_unbox_string(b);
        char* result = kore_str_concat(sa, sb);
        return (int64_t)kore_box_string(result);
    }
    
    // RAW POINTER STRING HANDLING
    // Heuristic: values >= 0x10000000000 (64GB) are likely pointers, not small integers
    // TEXT segment pointers are typically in 0x7FF... range (Windows)
    // Heap pointers are typically in 0x1xx... - 0x3xx... range
    #define LIKELY_POINTER_MIN 0x10000000000ULL  // 64GB - very unlikely to be an integer
    
    // Check if both look like pointers (not small integers)
    int a_looks_like_ptr = (a > LIKELY_POINTER_MIN && a < NANBOX_QNAN);
    int b_looks_like_ptr = (b > LIKELY_POINTER_MIN && b < NANBOX_QNAN);
    
    // If both look like pointers and neither is a tagged integer, try string concat
    if (a_looks_like_ptr && b_looks_like_ptr && !kore_is_tagged(a) && !kore_is_tagged(b)) {
        const char* sa = (const char*)a;
        const char* sb = (const char*)b;
        char* result = kore_str_concat(sa, sb);
        return (int64_t)kore_box_string(result);
    }
    
    // One is tagged string, other is raw pointer
    if (kore_is_string(a) && b_looks_like_ptr && !kore_is_tagged(b)) {
        const char* sa = kore_unbox_string(a);
        const char* sb = (const char*)b;
        char* result = kore_str_concat(sa, sb);
        return (int64_t)kore_box_string(result);
    }
    if (kore_is_string(b) && a_looks_like_ptr && !kore_is_tagged(a)) {
        const char* sa = (const char*)a;
        const char* sb = kore_unbox_string(b);
        char* result = kore_str_concat(sa, sb);
        return (int64_t)kore_box_string(result);
    }
    
    // Auto-unbox each operand independently for V1 compatibility
    int64_t a_raw, b_raw;
    if (kore_is_int(a)) a_raw = kore_unbox_int(a);
    else if (a < NANBOX_QNAN) a_raw = a_val;
    else a_raw = 0; // Invalid
    
    if (kore_is_int(b)) b_raw = kore_unbox_int(b);
    else if (b < NANBOX_QNAN) b_raw = b_val;
    else b_raw = 0; // Invalid
    
    return (int64_t)kore_box_int(a_raw + b_raw);
}

int64_t kore_sub_op(int64_t a_val, int64_t b_val) {
    uint64_t a = (uint64_t)a_val;
    uint64_t b = (uint64_t)b_val;
    
    // Auto-unbox each operand independently for V1 compatibility
    int64_t a_raw, b_raw;
    
    if (kore_is_int(a)) {
        a_raw = kore_unbox_int(a);
    } else if (a_val < NANBOX_QNAN) {
        // Raw small integer
        a_raw = a_val;
    } else {
        // Assume double or other - just use raw
        a_raw = a_val;
    }
    
    if (kore_is_int(b)) {
        b_raw = kore_unbox_int(b);
    } else if (b_val < NANBOX_QNAN) {
        // Raw small integer
        b_raw = b_val;
    } else {
        // Assume double or other - just use raw
        b_raw = b_val;
    }
    
    // Return RAW result for V1 compatibility
    // V1 codegen uses raw integers for array indexing
    return a_raw - b_raw;
}

int64_t kore_mul_op(int64_t a_val, int64_t b_val) {
    uint64_t a = (uint64_t)a_val;
    uint64_t b = (uint64_t)b_val;
    
    // Auto-unbox each operand independently for V1 compatibility
    int64_t a_raw, b_raw;
    
    if (kore_is_int(a)) {
        a_raw = kore_unbox_int(a);
    } else if (a_val < NANBOX_QNAN) {
        // Raw small integer
        a_raw = a_val;
    } else {
        // Assume double or other - just use raw
        a_raw = a_val;
    }
    
    if (kore_is_int(b)) {
        b_raw = kore_unbox_int(b);
    } else if (b_val < NANBOX_QNAN) {
        // Raw small integer
        b_raw = b_val;
    } else {
        // Assume double or other - just use raw
        b_raw = b_val;
    }
    
    // Return RAW result for V1 compatibility
    return a_raw * b_raw;
}

int64_t kore_div_op(int64_t a_val, int64_t b_val) {
    uint64_t a = (uint64_t)a_val;
    uint64_t b = (uint64_t)b_val;
    
    if (kore_is_int(a) && kore_is_int(b)) {
        int64_t vb = kore_unbox_int(b);
        if (vb == 0) {
            fprintf(stderr, "PANIC: Division by zero\n");
            exit(1);
        }
        int64_t result = kore_unbox_int(a) / vb;
        return (int64_t)kore_box_int(result);
    }
    
    if (kore_is_double(a) && kore_is_double(b)) {
        double vb = kore_unbox_double(b);
        if (vb == 0.0) {
             // IEEE 754 infinity? Or panic?
             // For now, let's just do it
        }
        double result = kore_unbox_double(a) / vb;
        return (int64_t)kore_box_double(result);
    }
    
    if (b_val == 0) return 0;
    return a_val / b_val;
}

int64_t kore_rem_op(int64_t a_val, int64_t b_val) {
    uint64_t a = (uint64_t)a_val;
    uint64_t b = (uint64_t)b_val;
    
    if (kore_is_int(a) && kore_is_int(b)) {
        int64_t vb = kore_unbox_int(b);
        if (vb == 0) {
            fprintf(stderr, "PANIC: Remainder by zero\n");
            exit(1);
        }
        int64_t result = kore_unbox_int(a) % vb;
        return (int64_t)kore_box_int(result);
    }
    
    // Floats don't support % in C, would need fmod
    
    if (b_val == 0) {
        fprintf(stderr, "PANIC: Remainder by zero (legacy)\n");
        exit(1);
    }
    return a_val % b_val;
}

// Comparison Helpers - Return RAW booleans for V1 compatibility

// Helper to unbox integer with V1 compatibility
static int64_t unbox_int_v1(uint64_t val, int64_t raw_val) {
    if (kore_is_int(val)) {
        return kore_unbox_int(val);
    } else if (raw_val < NANBOX_QNAN) {
        return raw_val;  // Already raw
    }
    return raw_val;  // Fallback
}

int64_t kore_lt_op(int64_t a_val, int64_t b_val) {
    int64_t a = unbox_int_v1((uint64_t)a_val, a_val);
    int64_t b = unbox_int_v1((uint64_t)b_val, b_val);
    return a < b ? 1 : 0;  // Raw boolean
}

int64_t kore_gt_op(int64_t a_val, int64_t b_val) {
    int64_t a = unbox_int_v1((uint64_t)a_val, a_val);
    int64_t b = unbox_int_v1((uint64_t)b_val, b_val);
    return a > b ? 1 : 0;  // Raw boolean
}

int64_t kore_le_op(int64_t a_val, int64_t b_val) {
    int64_t a = unbox_int_v1((uint64_t)a_val, a_val);
    int64_t b = unbox_int_v1((uint64_t)b_val, b_val);
    return a <= b ? 1 : 0;  // Raw boolean
}

int64_t kore_ge_op(int64_t a_val, int64_t b_val) {
    int64_t a = unbox_int_v1((uint64_t)a_val, a_val);
    int64_t b = unbox_int_v1((uint64_t)b_val, b_val);
    return a >= b ? 1 : 0;  // Raw boolean
}

int64_t kore_eq_op(int64_t a_val, int64_t b_val) {
    // Fast path: bit-identical
    if (a_val == b_val) return 1;
    
    uint64_t a = (uint64_t)a_val;
    uint64_t b = (uint64_t)b_val;

    // If both are integers (boxed or raw), compare them
    if (kore_is_int(a) && kore_is_int(b)) {
        return kore_unbox_int(a) == kore_unbox_int(b);
    }
    
    // V1 compatibility: unbox integers before comparing
    int64_t ua = unbox_int_v1(a, a_val);
    int64_t ub = unbox_int_v1(b, b_val);
    if (ua == ub && (ua != a_val || ub != b_val)) {
        return 1;
    }
    
    // Only fall back to string comparison if both are potentially strings
    if (kore_is_string(a) || kore_is_string(b) || (a < NANBOX_QNAN && a > 0x10000) || (b < NANBOX_QNAN && b > 0x10000)) {
        // kore_str_eq returns KORE_TRUE/KORE_FALSE, we need truthy check
        int64_t res = kore_str_eq(a_val, b_val);
        return kore_is_truthy(res);
    }

    return 0;
}

int64_t kore_neq_op(int64_t a_val, int64_t b_val) {
    return kore_eq_op(a_val, b_val) ? 0 : 1;
}

int64_t kore_ord(int64_t str_val) {
    const char* str = (const char*)kore_unbox_any_ptr(str_val);
    if (!str || !*str) return (int64_t)kore_box_int(0);
    return (int64_t)kore_box_int(str[0]);
}

int64_t kore_chr(int64_t code_val) {
    // Auto-unbox NaN-boxed integer
    int64_t code = kore_is_int((uint64_t)code_val) ? kore_unbox_int((uint64_t)code_val) : code_val;
    
    char* str = (char*)malloc(2);
    if (!str) return 0;
    str[0] = (char)code;
    str[1] = '\0';
    return (int64_t)kore_box_string(str);
}

// Get character code at index in string (for efficient lexer)
int64_t kore_char_code_at(int64_t str_val, int64_t index_val) {
    // Auto-unbox NaN-boxed string and index
    const char* str = (const char*)kore_unbox_any_ptr(str_val);
    
    int64_t index = kore_is_int((uint64_t)index_val) ? kore_unbox_int((uint64_t)index_val) : index_val;
    
    if (!str) return (int64_t)kore_box_int(0);
    size_t len = strlen(str);
    if (index < 0 || (size_t)index >= len) return (int64_t)kore_box_int(0);
    return (int64_t)kore_box_int((unsigned char)str[index]);
}

// Create single-character string from code (for efficient lexer)  
int64_t kore_char_from_code(int64_t code_val) {
    // Auto-unbox NaN-boxed integer
    int64_t code = kore_is_int((uint64_t)code_val) ? kore_unbox_int((uint64_t)code_val) : code_val;
    
    char* str = (char*)arena_alloc(2);
    str[0] = (char)code;
    str[1] = '\0';
    return (int64_t)kore_box_string(str);
}

// Get single character at index as string (for compatibility)
int64_t kore_char_at(int64_t str_val, int64_t index_val) {
    const char* str = kore_is_string((uint64_t)str_val) ? kore_unbox_string((uint64_t)str_val) : (const char*)str_val;
    int64_t index = kore_is_int((uint64_t)index_val) ? kore_unbox_int((uint64_t)index_val) : index_val;
    
    if (!str) return (int64_t)kore_box_string(kore_str_new(""));
    size_t len = strlen(str);
    if (index < 0 || (size_t)index >= len) return (int64_t)kore_box_string(kore_str_new(""));
    
    char buf[2];
    buf[0] = str[index];
    buf[1] = '\0';
    return (int64_t)kore_box_string(kore_str_new(buf));
}

// =============================================================================
// Array Operations (heap-allocated dynamic arrays)
// =============================================================================

typedef struct {
    int64_t* data;
    int64_t len;
    int64_t cap;
} KoreArray;

// We store arrays as pointers cast to i64
static int array_new_count = 0;
int64_t kore_array_new() {
    array_new_count++;
    
    KoreArray* arr = (KoreArray*)arena_alloc(sizeof(KoreArray));
    arr->data = NULL;
    arr->len = 0;
    arr->cap = 0;
    return (int64_t)arr;
}

int64_t kore_array_push(int64_t arr_val, int64_t value) {
    KoreArray* arr = (KoreArray*)kore_unbox_any_ptr(arr_val);
    if (!arr) return 0;
    
    if (arr->len >= arr->cap) {
        int64_t new_cap = arr->cap == 0 ? 8 : arr->cap * 2;
        arr->data = (int64_t*)realloc(arr->data, new_cap * sizeof(int64_t));
        if (!arr->data) {
            fprintf(stderr, "FATAL: OOM in kore_array_push\n");
            exit(1);
        }
        arr->cap = new_cap;
    }

    if (!arr->data) {
         fprintf(stderr, "FATAL: arr->data is NULL even after realloc! cap=%lld\n", arr->cap);
         exit(1);
    }

    arr->data[arr->len] = value;
    arr->len++;
    return arr_val;
}

int64_t kore_array_pop(int64_t arr_val) {
    KoreArray* arr = (KoreArray*)kore_unbox_any_ptr(arr_val);
    if (!arr) return 0;
    
    if (arr->len == 0) return 0;
    return arr->data[--arr->len];
}

static int array_get_count = 0;
int64_t kore_array_get(int64_t arr_val, int64_t index_val) {
    array_get_count++;
    
    // Auto-unbox NaN-boxed array pointer
    KoreArray* arr = (KoreArray*)kore_unbox_any_ptr(arr_val);
    if (!arr) return 0;
    
    // Auto-unbox NaN-boxed index
    int64_t index = (index_val < NANBOX_QNAN) ? index_val : kore_unbox_int((uint64_t)index_val);
    
    
    // Comprehensive bounds checking with diagnostic info
    if (index < 0 || index >= arr->len) {
        fprintf(stderr, "\n═══════════════════════════════════════════════════════════\n");
        fprintf(stderr, "ERROR: Array index out of bounds\n");
       fprintf(stderr, "═══════════════════════════════════════════════════════════\n\n");
        
        fprintf(stderr, "Array Information:\n");
        fprintf(stderr, "  Address:  %p\n", (void*)arr);
        fprintf(stderr, "  Length:   %lld\n", (long long)arr->len);
        fprintf(stderr, "  Capacity: %lld\n", (long long)arr->cap);
        fprintf(stderr, "  Data ptr: %p\n", (void*)arr->data);
        
        fprintf(stderr, "\nIndex Information:\n");
        fprintf(stderr, "  Requested index: %lld\n", (long long)index);
        fprintf(stderr, "  Index (raw):     0x%llx\n", (unsigned long long)index_val);
        if (index_val >= NANBOX_QNAN) {
            fprintf(stderr, "  Index (boxed):   0x%llx (NaN-boxed integer)\n", (unsigned long long)index_val);
        }
        fprintf(stderr, "  Valid range:     0 <= index < %lld\n", (long long)arr->len);
        
        if (arr->len > 0 && arr->data) {
            fprintf(stderr, "\nArray Contents Preview:\n");
            int show_count = arr->len < 5 ? (int)arr->len : 5;
            for (int i = 0; i < show_count; i++) {
                fprintf(stderr, "  arr[%d] = 0x%llx\n", i, (unsigned long long)arr->data[i]);
            }
            if (arr->len > 5) {
                fprintf(stderr, "  ... (%lld more elements)\n", (long long)(arr->len - 5));
            }
        }
        
        fprintf(stderr, "\nLikely Causes:\n");
        if (index == arr->len) {
            fprintf(stderr, "   Index equals length - off-by-one error\n");
            fprintf(stderr, "    - Using 1-based indexing instead of 0-based\n");
            fprintf(stderr, "    - Loop condition should be 'i < len' not 'i <= len'\n");
        } else if (index == index_val && index_val >= NANBOX_QNAN) {
            fprintf(stderr, "   Using boxed integer directly as index\n");
            fprintf(stderr, "    - Variable assignment may not be storing computed value\n");
            fprintf(stderr, "    - Let-binding codegen bug in bootstrap compiler\n");
            fprintf(stderr, "    - Expression result not being used\n");
        } else if (index > arr->len + 100) {
            fprintf(stderr, "   Index is very large - possible memory corruption\n");
            fprintf(stderr, "    - Uninitialized variable\n");
            fprintf(stderr, "    - Pointer arithmetic error\n");
        } else {
            fprintf(stderr, "  - Check loop bounds and index calculations\n");
            fprintf(stderr, "  - Verify array was populated correctly\n");
        }
        
        fprintf(stderr, "\nStack Trace:\n");
        kore_print_stack_trace();
        fprintf(stderr, "═══════════════════════════════════════════════════════════\n\n");
        fflush(stderr);
        exit(1);
    }
    
    return ((int64_t*)arr->data)[index];
}

void kore_array_set(int64_t arr_val, int64_t index_val, int64_t value) {
    // Auto-unbox NaN-boxed array pointer and index
    KoreArray* arr = (KoreArray*)kore_unbox_any_ptr(arr_val);
    if (!arr) return;
    
    int64_t index = kore_is_int((uint64_t)index_val) ? kore_unbox_int((uint64_t)index_val) : index_val;
    
    if (index < 0 || index >= arr->len) {
        fprintf(stderr, "FATAL: Array SET index out of bounds: %lld (len=%lld)\n", 
                (long long)index, (long long)arr->len);
        exit(1);
    }
    arr->data[index] = value;
}

static int array_len_count = 0;
int64_t kore_array_len(int64_t arr_val) {
    array_len_count++;

    KoreArray* arr = (KoreArray*)kore_unbox_any_ptr(arr_val);
    if (!arr) return 0;

    // Return boxed integer for V1/V2 compatibility
    return (int64_t)kore_box_int(arr->len);
}


void kore_array_free(int64_t arr_ptr) {
    KoreArray* arr = (KoreArray*)arr_ptr;
    if (arr->data) free(arr->data);
    free(arr);
}

// =============================================================================
// Helper Functions (Stdlib)
// =============================================================================

// String substring search (strstr-based)
int64_t kore_str_contains(int64_t str_ptr, int64_t substr_ptr) {
    if (str_ptr == 0 || substr_ptr == 0) return 0;
    const char* str = (const char*)str_ptr;
    const char* substr = (const char*)substr_ptr;
    return strstr(str, substr) != NULL ? 1 : 0;
}

// Array membership check (internal raw return)
int64_t kore_array_contains(int64_t arr_ptr, int64_t item_val) {
    if (arr_ptr == 0) return 0;
    KoreArray* arr = (KoreArray*)arr_ptr;
    
    for (int64_t i = 0; i < arr->len; i++) {
        if (kore_str_eq(arr->data[i], item_val) == KORE_TRUE) {
            return 1;
        }
    }
    return 0;
}

// Polymorphic contains: detects string vs array using NaN-boxing
int64_t kore_contains(int64_t first_val, int64_t second_val) {
    if (first_val == 0) return (int64_t)kore_box_bool(0);
    
    uint64_t first = (uint64_t)first_val;
    uint64_t second = (uint64_t)second_val;
    int res = 0;
    
    if (kore_is_string(first)) {
        const char* str = kore_unbox_string(first);
        const char* substr = kore_is_string(second) ? kore_unbox_string(second) : (const char*)second_val;
        res = (str && substr && strstr(str, substr) != NULL);
    } else if (kore_is_ptr(first)) {
        // NaN-boxed pointer
        KoreArray* arr = (KoreArray*)kore_unbox_ptr(first);
        for (int64_t i = 0; i < arr->len; i++) {
            if (kore_str_eq(arr->data[i], second_val) == KORE_TRUE) {
                res = 1;
                break;
            }
        }
    } else if (!kore_is_tagged(first) && first > 0x10000) {
        // RAW pointer fallback (for arrays from kore_array_new which return raw pointers)
        // Heuristic: value > 0x10000 and not tagged -> likely a heap pointer
        KoreArray* arr = (KoreArray*)first_val;
        printf("DEBUG contains: raw arr=%p, len=%lld, looking for second_val=%llx\n", 
               (void*)arr, (long long)arr->len, (unsigned long long)second_val);
        for (int64_t i = 0; i < arr->len; i++) {
            printf("DEBUG contains[%lld]: arr->data[i]=%llx\n", (long long)i, (unsigned long long)arr->data[i]);
            if (kore_str_eq(arr->data[i], second_val) == KORE_TRUE) {
                res = 1;
                break;
            }
        }
    }

    
    return (int64_t)kore_box_bool(res);
}


int64_t kore_split(int64_t str_val, int64_t delim_val) {
    const char* str = kore_is_string((uint64_t)str_val) ? kore_unbox_string((uint64_t)str_val) : (const char*)str_val;
    const char* delim = kore_is_string((uint64_t)delim_val) ? kore_unbox_string((uint64_t)delim_val) : (const char*)delim_val;
    
    // debug logging removed

    int64_t arr_boxed = kore_array_new();
    
    if (!str || !delim) return arr_boxed;
    
    if (strlen(delim) == 0) {
        size_t len = strlen(str);
        for (size_t i = 0; i < len; i++) {
            char buf[2];
            buf[0] = str[i];
            buf[1] = '\0';
            kore_array_push(arr_boxed, (int64_t)kore_box_string(kore_str_new(buf)));
        }
    } else {
        char* str_copy = strdup(str);
        char* token = strtok(str_copy, delim);
        while (token != NULL) {
            kore_array_push(arr_boxed, (int64_t)kore_box_string(kore_str_new(token)));
            token = strtok(NULL, delim);
        }
        free(str_copy);
    }
    
    return arr_boxed;
}

int64_t kore_len(int64_t obj_val) {
    if (obj_val == 0) return (int64_t)kore_box_int(0);
    uint64_t uval = (uint64_t)obj_val;
    
    if (kore_is_string(uval)) {
        return (int64_t)kore_box_int((int64_t)strlen(kore_unbox_string(uval)));
    } else if (kore_is_ptr(uval)) {
        KoreArray* arr = (KoreArray*)kore_unbox_ptr(uval);
        return (int64_t)kore_box_int(arr->len);
    }
    
    // Fallback if not boxed correctly
    return (int64_t)kore_box_int(0);
}

int64_t kore_to_int(int64_t str_val) {
    const char* str = kore_is_string((uint64_t)str_val) ? kore_unbox_string((uint64_t)str_val) : (const char*)str_val;
    if (!str) return (int64_t)kore_box_int(0);
    return (int64_t)kore_box_int(atoll(str));
}

// Float logic is tricky because we use i64 everywhere in this bootstrap.
// We might be storing float bits in i64?
// parser.kr uses Expr::Float(float(tok.lexeme)).
// If Expr::Float stores i64, then we just cast bits?
// But let's assume standard float parsing for now, returning bits as i64?
// Or maybe we just return integer part?
// The bootstrap uses i64 for everything.
// I'll leave float for now or return 0.
int64_t kore_to_float(int64_t str_val) {
    const char* str = kore_is_string((uint64_t)str_val) ? kore_unbox_string((uint64_t)str_val) : (const char*)str_val;
    if (!str) return 0;
    double d = atof(str);
    return (int64_t)kore_box_double(d);
}

int64_t kore_to_string(int64_t val) {
    uint64_t uval = (uint64_t)val;
    char buf[128];
    
    // V1 COMPATIBILITY: Handle raw integers first (before tag checks)
    // Raw integers from V1 are small values < NANBOX_QNAN
    if (uval < NANBOX_QNAN && uval < 0x0010000000000000ULL) {
        // This is a raw integer (including 0)
        sprintf(buf, "%lld", (long long)val);
        return (int64_t)kore_box_string(kore_str_new(buf));
    }
    
    uint64_t tag = kore_get_tag(uval);
    
    if (kore_is_string(uval)) {
        return val;
    } else if (tag == KORE_TAG_INT) {
        int64_t unboxed = kore_unbox_int(uval);
        sprintf(buf, "%lld", (long long)unboxed);
    } else if (tag == (uint64_t)-1) { // Double
        sprintf(buf, "%g", kore_unbox_double(uval));
    } else if (tag == KORE_TAG_BOOL) {
        sprintf(buf, "%s", kore_unbox_bool(uval) ? "true" : "false");
    } else if (tag == KORE_TAG_NULL) {
        sprintf(buf, "null");
    } else if (kore_is_ptr(uval)) {
        sprintf(buf, "[Ptr %p]", (void*)kore_unbox_ptr(uval));
    } else {
        // Fallback for raw pointers/values
        sprintf(buf, "%lld", (long long)val);
    }
    
    return (int64_t)kore_box_string(kore_str_new(buf));
}

int64_t kore_range(int64_t start_val, int64_t end_val) {
    int64_t start = kore_is_int((uint64_t)start_val) ? kore_unbox_int((uint64_t)start_val) : start_val;
    int64_t end = kore_is_int((uint64_t)end_val) ? kore_unbox_int((uint64_t)end_val) : end_val;
    
    int64_t arr_boxed = kore_array_new();
    for (int64_t i = start; i < end; i++) {
        kore_array_push(arr_boxed, (int64_t)kore_box_int(i));
    }
    return arr_boxed;
}

int64_t kore_substring(int64_t str_val, int64_t start_val, int64_t end_val) {
    // Auto-unbox NaN-boxed string
    const char* str;
    if (kore_is_string((uint64_t)str_val)) {
        str = kore_unbox_string((uint64_t)str_val);
    } else {
        str = (const char*)str_val;
    }
    
    // Auto-unbox NaN-boxed integers
    int64_t start = kore_is_int((uint64_t)start_val) ? kore_unbox_int((uint64_t)start_val) : start_val;
    int64_t end = kore_is_int((uint64_t)end_val) ? kore_unbox_int((uint64_t)end_val) : end_val;
    
    size_t len = strlen(str);
    if (start < 0) start = 0;
    if (end > (int64_t)len) end = len;
    if (start >= end) return (int64_t)kore_box_string(kore_str_new(""));
    
    size_t sub_len = end - start;
    char* result = (char*)arena_alloc(sub_len + 1);
    strncpy(result, str + start, sub_len);
    result[sub_len] = '\0';
    return (int64_t)kore_box_string(result);
}

int64_t kore_str_ends_with(int64_t str_val, int64_t suffix_val) {
    const char* str = kore_is_string((uint64_t)str_val) ? kore_unbox_string((uint64_t)str_val) : (const char*)str_val;
    const char* suffix = kore_is_string((uint64_t)suffix_val) ? kore_unbox_string((uint64_t)suffix_val) : (const char*)suffix_val;
    
    if (!str || !suffix) return (int64_t)kore_box_bool(0);
    
    size_t str_len = strlen(str);
    size_t suffix_len = strlen(suffix);
    
    if (suffix_len > str_len) return (int64_t)kore_box_bool(0);
    
    return (int64_t)kore_box_bool(strcmp(str + str_len - suffix_len, suffix) == 0);
}

int64_t kore_slice(int64_t arr_val, int64_t start_val, int64_t end_val) {
    // Auto-unbox NaN-boxed pointer and integers
    KoreArray* arr;
    if (kore_is_ptr((uint64_t)arr_val)) {
        arr = (KoreArray*)kore_unbox_ptr((uint64_t)arr_val);
    } else {
        arr = (KoreArray*)arr_val;
    }
    
    int64_t start = kore_is_int((uint64_t)start_val) ? kore_unbox_int((uint64_t)start_val) : start_val;
    int64_t end = kore_is_int((uint64_t)end_val) ? kore_unbox_int((uint64_t)end_val) : end_val;
    
    int64_t new_arr = kore_array_new();
    if (start < 0) start = 0;
    if (end > arr->len) end = arr->len;
    for (int64_t i = start; i < end; i++) {
        kore_array_push(new_arr, arr->data[i]);
    }
    return new_arr;
}

int64_t kore_append(int64_t str_val1, int64_t str_val2) {
    // Auto-unbox NaN-boxed strings
    const char* a;
    const char* b;
    if (kore_is_string((uint64_t)str_val1)) {
        a = kore_unbox_string((uint64_t)str_val1);
    } else {
        a = (const char*)str_val1;
    }
    if (kore_is_string((uint64_t)str_val2)) {
        b = kore_unbox_string((uint64_t)str_val2);
    } else {
        b = (const char*)str_val2;
    }
    return (int64_t)kore_box_string(kore_str_concat(a, b));
}

// =============================================================================
// Option/Box Helpers
// =============================================================================

// Option is represented as { tag: i64, value: i64 }
// tag = 0 for None, 1 for Some

typedef struct {
    int64_t tag;
    int64_t value;
    int64_t name; // Variant name
} KoreOption;

int64_t kore_some(int64_t value) {
    KoreOption* opt = (KoreOption*)arena_alloc(sizeof(KoreOption));
    opt->tag = 0; // Some is 1st variant
    // Allocate tuple for payload to match generic EnumVariant layout
    int64_t* tuple = (int64_t*)arena_alloc(sizeof(int64_t));
    tuple[0] = value;
    opt->value = (int64_t)tuple;
    
    opt->name = (int64_t)kore_box_string(kore_str_new("Some"));
    return (int64_t)kore_box_ptr(opt);
}

int64_t kore_none() {
    KoreOption* opt = (KoreOption*)arena_alloc(sizeof(KoreOption));
    opt->tag = 1; // None is 2nd variant
    opt->value = 0; // Null tuple
    opt->name = (int64_t)kore_box_string(kore_str_new("None"));
    return (int64_t)kore_box_ptr(opt);
}

int64_t kore_unwrap(int64_t opt_val) {
    if (opt_val == 0) {
        fprintf(stderr, "PANIC: unwrap called on null pointer\n");
        exit(1);
    }
    KoreOption* opt;
    if (kore_is_ptr((uint64_t)opt_val)) {
        opt = (KoreOption*)kore_unbox_ptr((uint64_t)opt_val);
    } else {
        opt = (KoreOption*)opt_val;
    }
    
    if (opt->tag == 1) {  // tag 1 = None, tag 0 = Some
        fprintf(stderr, "PANIC: called unwrap on None\n");
        exit(1);
    }
    // value is a tuple pointer
    int64_t* tuple = (int64_t*)opt->value;
    return tuple[0];
}

// Box is just a wrapper around a value (heap-allocated)
int64_t kore_box(int64_t value) {
    int64_t* box = (int64_t*)arena_alloc(sizeof(int64_t));
    *box = value;
    return (int64_t)box;
}

int64_t kore_unbox(int64_t box_ptr) {
    int64_t* box = (int64_t*)box_ptr;
    return *box;
}

// =============================================================================
// Memory Management (Already defined above)
// =============================================================================

// void* kore_alloc(int64_t size) - defined in arena section
// void kore_free(void* ptr) - defined in arena section

// =============================================================================
// Type Tags (for dynamic typing / enums) - LEGACY, use different prefix to avoid NaN-boxing conflict
// =============================================================================

#define KORE_VALUE_TAG_INT    0
#define KORE_VALUE_TAG_FLOAT  1
#define KORE_VALUE_TAG_STRING 2
#define KORE_VALUE_TAG_BOOL   3
#define KORE_VALUE_TAG_ARRAY  4
#define KORE_VALUE_TAG_NONE   5

typedef struct {
    int64_t tag;
    int64_t value;  // or pointer cast to i64
} KoreValue;

int64_t kore_value_tag(int64_t value_ptr) {
    KoreValue* v = (KoreValue*)value_ptr;
    return v->tag;
}

int64_t kore_value_data(int64_t value_ptr) {
    KoreValue* v = (KoreValue*)value_ptr;
    return v->value;
}

// =============================================================================
// File I/O
// =============================================================================

char* kore_file_read(const char* path) {
    // // fprintf(stderr, "[file_read] Reading '%s'\n", path ? path : "NULL");
    // // fflush(stderr);
    FILE* f = fopen(path, "rb");
    if (!f) {
        // // fprintf(stderr, "[file_read] Failed to open '%s'\n", path ? path : "NULL");
        // // fflush(stderr);
        return NULL;
    }
    
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    
    // // fprintf(stderr, "[file_read] Size: %ld\n", size);
    // // fflush(stderr);

    char* content = (char*)malloc(size + 1);
    long read_size = fread(content, 1, size, f);
    (void)read_size;  // Silence unused variable warning
    // // fprintf(stderr, "[file_read] Read %ld bytes\n", read_size);
    // // fflush(stderr);

    content[size] = '\0';
    fclose(f);
    
    return content;
}

int64_t kore_file_write(const char* path, const char* content) {
    // // fprintf(stderr, "[file_write] path='%s' content_len=%zu\n", path ? path : "NULL", content ? strlen(content) : 0);
    // // fflush(stderr);
    FILE* f = fopen(path, "wb");
    if (!f) {
        // // fprintf(stderr, "[file_write] Failed to open file!\n");
        // // fflush(stderr);
        return 0;
    }
    
    size_t len = strlen(content);
    size_t written = fwrite(content, 1, len, f);
    (void)written;  // Silence unused variable warning
    // // fprintf(stderr, "[file_write] Wrote %zu bytes\n", written);
    // // fflush(stderr);
    fclose(f);
    
    return 1;
}

// =============================================================================
// Map Operations (simple key-value store using parallel arrays)
// =============================================================================

typedef struct {
    int64_t* keys;
    int64_t* values;
    int64_t len;
    int64_t cap;
} KoreMap;

int64_t Map_new() {
    KoreMap* map = (KoreMap*)malloc(sizeof(KoreMap));
    map->keys = NULL;
    map->values = NULL;
    map->len = 0;
    map->cap = 0;
    return (int64_t)kore_box_ptr(map);
}

int64_t kore_contains_key(int64_t map_val, int64_t key_val) {
    KoreMap* map = (KoreMap*)kore_unbox_any_ptr(map_val);
    if (!map) return (int64_t)kore_box_bool(0);
    const char* key = (const char*)kore_unbox_any_ptr(key_val);
    if (map->keys == NULL) return (int64_t)kore_box_bool(0);

    for (int64_t i = 0; i < map->len; i++) {
        int64_t stored_key_val = map->keys[i];
        const char* stored_key = (const char*)kore_unbox_any_ptr(stored_key_val);
        if (stored_key && strcmp(stored_key, key) == 0) {
            return (int64_t)kore_box_bool(1);
        }
    }
    return (int64_t)kore_box_bool(0);
}

// Map lookup - returns 0 if not found
int64_t kore_map_get(int64_t map_val, int64_t key_val) {
    KoreMap* map = (KoreMap*)kore_unbox_any_ptr(map_val);
    if (!map) return (int64_t)kore_box_null();
    const char* key = (const char*)kore_unbox_any_ptr(key_val);
    if (!map->keys) return (int64_t)kore_box_null();
    for (int64_t i = 0; i < map->len; i++) {
        int64_t stored_key_val = map->keys[i];
        const char* stored_key = (const char*)kore_unbox_any_ptr(stored_key_val);
        if (stored_key && strcmp(stored_key, key) == 0) {
            return map->values[i];
        }
    }
    return (int64_t)kore_box_null();
}

// Map insert/update
void kore_map_set(int64_t map_val, int64_t key_val, int64_t value) {
    KoreMap* map = (KoreMap*)kore_unbox_any_ptr(map_val);
    if (!map) return;
    const char* key = (const char*)kore_unbox_any_ptr(key_val);
    
    // Check if key exists
    for (int64_t i = 0; i < map->len; i++) {
        int64_t stored_key_val = map->keys[i];
        const char* stored_key = (const char*)kore_unbox_any_ptr(stored_key_val);
        if (stored_key && strcmp(stored_key, key) == 0) {
            map->values[i] = value;
            return;
        }
    }
    
    // Add new key
    if (map->len >= map->cap) {
        int64_t new_cap = map->cap == 0 ? 8 : map->cap * 2;
        map->keys = (int64_t*)realloc(map->keys, new_cap * sizeof(int64_t));
        map->values = (int64_t*)realloc(map->values, new_cap * sizeof(int64_t));
        if (!map->keys || !map->values) {
             printf("FATAL: OOM in map_set\n");
             exit(1);
        }
        map->cap = new_cap;
    }
    map->keys[map->len] = key_val;
    map->values[map->len] = value;
    map->len++;
}

// =============================================================================
// String Join (for StringBuilder.build())
// =============================================================================

int64_t kore_join(int64_t arr_val, int64_t delim_val) {
    KoreArray* arr = (KoreArray*)kore_unbox_any_ptr(arr_val);
    const char* delim = (const char*)kore_unbox_any_ptr(delim_val);
    if (!delim) delim = "";

    if (!arr || arr->len == 0) {
        return (int64_t)kore_box_string(kore_str_new(""));
    }
    
    size_t delim_len = strlen(delim);
    size_t total_len = 0;
    for (int64_t i = 0; i < arr->len; i++) {
        int64_t val = arr->data[i];
        const char* s = kore_is_string((uint64_t)val) ? kore_unbox_string((uint64_t)val) : (const char*)val;
        if (s) total_len += strlen(s);
        if (i < arr->len - 1) total_len += delim_len;
    }
    
    char* result = (char*)arena_alloc(total_len + 1);
    result[0] = '\0';
    
    for (int64_t i = 0; i < arr->len; i++) {
        int64_t val = arr->data[i];
        const char* s = kore_is_string((uint64_t)val) ? kore_unbox_string((uint64_t)val) : (const char*)val;
        if (s) strcat(result, s);
        if (i < arr->len - 1) strcat(result, delim);
    }
    
    return (int64_t)kore_box_string(result);
}

// Debug helper: Read 8 bytes from address
int64_t kore_peek(int64_t ptr) {
    if (ptr < 1000) {
        fprintf(stderr, "FATAL: kore_peek called with null/invalid pointer: %lld\n", (long long)ptr);
        return 0;
    }
    int64_t* p = (int64_t*)ptr;
    return *p;
}

// =============================================================================
// Variant Introspection (for enum pattern matching in interpreted code)
// =============================================================================

// Get tag name from a tagged union (simplified - returns tag number as string)
int64_t kore_variant_of(int64_t value_val) {
    // In our representation, enums are { tag: i64, payload: i8*, name: i8* }
    int64_t* ptr;
    if (kore_is_ptr((uint64_t)value_val)) {
        ptr = (int64_t*)kore_unbox_ptr((uint64_t)value_val);
    } else {
        ptr = (int64_t*)value_val;
    }
    
    if (!ptr) {
        // Return tagged string "None"
        return (int64_t)kore_box_string(kore_str_new("None"));
    }
    
    // Name is at offset 2 (the 3rd field)
    uint64_t name_val = (uint64_t)ptr[2];
    
    // If it's already a tagged string, return it
    if (kore_is_string(name_val)) {
        return (int64_t)name_val;
    }
    
    const char* name = (const char*)name_val;
    // If name is null (shouldn't happen for valid enums), fallback to tag
    if (name == NULL) {
        int64_t tag = *ptr;
        char buf[32];
        sprintf(buf, "%lld", (long long)tag);
        return (int64_t)kore_box_string(kore_str_new(buf));
    }
    
    // Create new string and NAN-BOX IT properly
    // This ensures it is treated as a String by println/str_eq/etc.
    // kore_box_string now uses KORE_BOX_STR macro to ensure correct tagging
    return (int64_t)kore_box_string(kore_str_new(name));
}

// Extract field from variant by index
int64_t kore_variant_field(int64_t value_val, int64_t field_idx_val) {
    int64_t* ptr;
    if (kore_is_ptr((uint64_t)value_val)) {
        ptr = (int64_t*)kore_unbox_ptr((uint64_t)value_val);
    } else {
        ptr = (int64_t*)value_val;
    }
    
    if (!ptr) return (int64_t)kore_box_null();
    
    // Unbox field index!
    int64_t field_idx = kore_is_int((uint64_t)field_idx_val) ? kore_unbox_int((uint64_t)field_idx_val) : field_idx_val;
    
    // The payload is at offset 1 (ptr[1])
    int64_t payload_val = ptr[1];
    if (payload_val == 0) return (int64_t)kore_box_null();
    
    int64_t* tuple;
    if (kore_is_ptr((uint64_t)payload_val)) {
        tuple = (int64_t*)kore_unbox_ptr((uint64_t)payload_val);
    } else {
        tuple = (int64_t*)payload_val;
    }
    
    return tuple[field_idx];
}

// =============================================================================
// Process / System
// =============================================================================

int64_t kore_system(const char* command) {
    return (int64_t)system(command);
}

void kore_exit(int64_t code) {
    exit((int)code);
}

void kore_panic(const char* message) {
    fprintf(stderr, "\n\n!!! KORE PANIC !!!\n");
    fprintf(stderr, "Reason: %s\n\n", message);
    fflush(stderr);  // CRITICAL: Force output before exit
    fflush(stdout);
    exit(1);
}

// =============================================================================
// Stdlib Wrappers (for direct linking without stdlib source)
// =============================================================================

int64_t args() {
    printf("DEBUG: [RUNTIME] Entering args()\n"); fflush(stdout);
    int64_t arr_boxed = kore_array_new();
    printf("DEBUG: [RUNTIME] g_argc = %d\n", g_argc); fflush(stdout);
    for (int i = 0; i < g_argc; i++) {
        printf("DEBUG: [RUNTIME] processing arg %d: %s\n", i, g_argv[i]); fflush(stdout);
        char* str_copy = kore_str_new(g_argv[i]);
        kore_array_push(arr_boxed, (int64_t)kore_box_string(str_copy));
    }
    printf("DEBUG: [RUNTIME] Returning from args()\n"); fflush(stdout);
    return arr_boxed;
}

int64_t read_file(int64_t path_val) {
    const char* p = (const char*)kore_unbox_any_ptr(path_val);
    printf("DEBUG: [RUNTIME] read_file: path='%s'\n", p ? p : "NULL"); fflush(stdout);
    char* content = kore_file_read(p);
    if (content == NULL) {
        printf("DEBUG: [RUNTIME] read_file FAILED\n"); fflush(stdout);
        return (int64_t)kore_box_null();
    }
    printf("DEBUG: [RUNTIME] read_file SUCCESS, len=%zu\n", strlen(content)); fflush(stdout);
    return (int64_t)kore_box_string(content); 
}

int64_t write_file(int64_t path_val, int64_t content_val) {
    const char* p = (const char*)kore_unbox_any_ptr(path_val);
    const char* c = (const char*)kore_unbox_any_ptr(content_val);
    return (int64_t)kore_box_int(kore_file_write(p, c));
}

int64_t file_exists(int64_t path_val) {
    const char* p = kore_is_string((uint64_t)path_val) ? kore_unbox_string((uint64_t)path_val) : (const char*)path_val;
    FILE* f = fopen(p, "rb");
    if (f) {
        fclose(f);
        return (int64_t)kore_box_int(1);
    }
    return (int64_t)kore_box_int(0);
}

int64_t substring(int64_t str_val, int64_t start, int64_t end) {
    return kore_substring(str_val, start, end);
}

int64_t replace(int64_t str_val, int64_t old_val, int64_t new_val) {
    return kore_str_replace(str_val, old_val, new_val);
}

int64_t starts_with(int64_t str_val, int64_t prefix_val) {
    return kore_str_starts_with(str_val, prefix_val);
}

// Wrappers for high-performance lexer
int64_t char_code_at(int64_t str_val, int64_t index_val) {
    return kore_char_code_at(str_val, index_val);
}

int64_t char_from_code(int64_t code_val) {
    return kore_char_from_code(code_val);
}

int64_t to_float(int64_t str_ptr) {
    return kore_to_float(str_ptr);
}

// Missing aliases for bootstrap compatibility
int64_t str_eq(int64_t a, int64_t b) {
    return kore_str_eq(a, b);
}

int64_t array_len(int64_t arr) {
    return kore_array_len(arr);
}

int64_t push(int64_t arr, int64_t val) {
    return kore_array_push(arr, val);
}

int64_t pop(int64_t arr) {
    return kore_array_pop(arr);
}

int64_t to_int(int64_t str_val) {
    return kore_to_int(str_val);
}

int64_t join(int64_t arr, int64_t sep) {
    return kore_join(arr, sep);
}

int64_t variant_of(int64_t val) {
    return kore_variant_of(val);
}

int64_t println(int64_t val) {
    return kore_println_str(val);
}

int64_t panic(int64_t msg) {
    const char* str = (const char*)kore_unbox_any_ptr(msg);
    kore_panic(str);
    return 0; // never reached
}

int64_t contains(int64_t str, int64_t substr) {
    return kore_contains(str, substr);  // Fixed: was kore_str_contains, should be polymorphic kore_contains
}

int64_t variant_field(int64_t val, int64_t idx) {
    return kore_variant_field(val, idx);
}

// Wrapper for stdlib compatibility - avoid name collision with C str* functions
int64_t kore_str(int64_t val) {
    return kore_to_string(val);
}

int64_t to_string(int64_t val) {
    return kore_to_string(val);
}

int64_t map_set(int64_t map, int64_t key, int64_t val) {
    kore_map_set(map, key, val);
    return 0;
}

int64_t contains_key(int64_t map, int64_t key) {
    return kore_contains_key(map, key);
}

int64_t map_get(int64_t map, int64_t key) {
    return kore_map_get(map, key);
}

int64_t split(int64_t str, int64_t delim) {
    return kore_split(str, delim);
}

int64_t str_len(int64_t str) {
    return kore_str_len(str);
}
extern int64_t main_kore();

// =============================================================================
// TokenType Constructors (Workaround for bootstrap codegen bug)
// =============================================================================

int64_t kore_create_token_simple(const char* name) {
    int64_t* ptr = (int64_t*)kore_alloc(24);
    ptr[0] = 0; 
    ptr[1] = 0; // null payload
    ptr[2] = (int64_t)kore_box_string(kore_str_new(name));
    return (int64_t)kore_box_ptr(ptr);
}

int64_t kore_create_token_payload(const char* name, int64_t val) {
    int64_t* ptr = (int64_t*)kore_alloc(24);
    ptr[0] = 0;
    int64_t* tuple = (int64_t*)kore_alloc(8);
    tuple[0] = val;
    ptr[1] = (int64_t)tuple;
    ptr[2] = (int64_t)kore_box_string(kore_str_new(name));
    return (int64_t)kore_box_ptr(ptr);
}

// =============================================================================
// Stack Trace Support
// =============================================================================

#define MAX_STACK_FRAMES 64

typedef struct {
    const char* function_name;
    const char* file;
    int line;
} KoreStackFrame;

static KoreStackFrame g_stack_frames[MAX_STACK_FRAMES];
static int g_stack_depth = 0;

// Called when entering a function (instrumented by codegen)
void kore_trace_enter(const char* func_name, const char* file, int line) {
    if (g_stack_depth < MAX_STACK_FRAMES) {
        g_stack_frames[g_stack_depth].function_name = func_name;
        g_stack_frames[g_stack_depth].file = file;
        g_stack_frames[g_stack_depth].line = line;
        g_stack_depth++;
    }
}

// Called when exiting a function
void kore_trace_exit(void) {
    if (g_stack_depth > 0) {
        g_stack_depth--;
    }
}

// Print stack trace on panic
void kore_print_stack_trace(void) {
    fprintf(stderr, "\n\033[1;36mStack trace (most recent call last):\033[0m\n");
    for (int i = g_stack_depth - 1; i >= 0; i--) {
        fprintf(stderr, "  at %s (%s:%d)\n",
            g_stack_frames[i].function_name,
            g_stack_frames[i].file,
            g_stack_frames[i].line);
    }
}

// Get current stack depth (for debugging)
int64_t kore_stack_depth(void) {
    return (int64_t)g_stack_depth;
}

int main(int argc, char** argv) {
    printf("DEBUG: [C-MAIN] argc=%d, argv[0]=%s\n", argc, argv[0]); fflush(stdout);
    kore_set_args(argc, argv);
    int64_t r = main_kore();
    return (int)r;
}

// Helper for debugging generated code assignment issues
void kore_debug_log_var(char* name, int64_t val) {
    // Unbox if necessary for display
    int64_t raw_val = val;
    int is_boxed = 0;
    if (val >= NANBOX_QNAN) {
        raw_val = kore_unbox_int((uint64_t)val);
        is_boxed = 1;
    }
    
    fprintf(stderr, "[DEBUG-LET] %s = %lld (0x%llx)%s\n", 
            name ? name : "???", 
            (long long)raw_val, 
            (unsigned long long)val,
            is_boxed ? " [BOXED]" : "");
    fflush(stderr);
}

// =============================================================================
// Compatibility Wrappers
// =============================================================================

int64_t char_at(int64_t str, int64_t idx) {
    return kore_char_at(str, idx);
}

int64_t dbg(int64_t val) {
    kore_debug_log_var("DBG", val);
    return val;
}

int64_t assert(int64_t cond, int64_t msg) {
    if (!kore_is_truthy(cond)) {
        const char* s = (const char*)kore_unbox_any_ptr(msg);
        kore_panic(s);
    }
    return cond;
}

int64_t now(void) {
    return 0; // Stub
}

int64_t sleep(int64_t ms) {
    return 0; // Stub
}

int64_t len(int64_t val) {
    return kore_len(val);
}

int64_t range(int64_t start, int64_t end) {
    return kore_range(start, end);
}

// Forward declare Map_new if needed or just alias whatever exposes it
extern int64_t Map_new(); 

int64_t map_new() {
    return Map_new();
}


