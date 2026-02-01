#include <stdio.h>
#include <string.h>
#include <stdint.h>

extern char* read_file(const char* path);

int64_t main_kore() {
    printf("Testing read_file...\n");
    char* s = read_file("src\\test_continue.kr");
    if (s) printf("Success: Read %zu bytes\n", strlen(s));
    else printf("Failed\n");
    return 0;
}
