#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>

int64_t test_main() {
    int64_t x = 10;
    int64_t y = 20;
    int64_t sum = x + y; // 30
    
    int64_t counter = 0;
    while (counter < 5) {
        counter = counter + 1;
    }
    // counter = 5
    
    int64_t total = 0;
    for (int64_t i = 1; i <= 5; i++) {
        total = total + i;
    }
    // total = 15
    
    int64_t* numbers = (int64_t*)malloc(5 * sizeof(int64_t));
    numbers[0] = 1;
    numbers[1] = 2;
    numbers[2] = 3;
    numbers[3] = 4;
    numbers[4] = 5;
    
    int64_t first = numbers[0]; // 1
    
    return sum + total + counter + first; // 30 + 15 + 5 + 1 = 51
}

int main() {
    int64_t result = test_main();
    printf("Result: %ld\n", result);
    return result;
}