module math

pub i32 add(i32 a, i32 b) {
    return a + b;
}

i32 helper() {
    return 1;
}

pub i32 add_with_helper(i32 a, i32 b) {
    return a + b + helper();
}
