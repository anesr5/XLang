module v0_2_loops

i32 countdown() {
    i32 n = 5;
    while n > 0 {
        if n == 1 {
            break;
        }
        n = n - 1;
        continue;
    }
    return n;
}

i32 main() {
    return countdown();
}
