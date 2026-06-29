module v0_2_demo

i32 sum_fixed_array() {
    i32[4] xs = { 1, 2, 3, 4 };
    i32 total = 0;
    i32 i = 0;
    while i < 4 {
        total = total + xs[i];
        i = i + 1;
    }
    return total;
}

i32 main() {
    return sum_fixed_array();
}
