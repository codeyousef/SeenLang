extern "C" int puts(const char *);

int main() {
    volatile int *value = new int(43);
    bool correct = *value == 43;
    delete value;
    if (!correct) return 1;
    puts("PASS: C++ driver alias links its language runtime");
    return 0;
}
