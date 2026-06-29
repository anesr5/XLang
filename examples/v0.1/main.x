module main

i32 add(i32 a, i32 b) {
    return a + b;
}

i32 clamp(i32 value, i32 min, i32 max) {
    if value < min {
        return min;
    }
    if value > max {
        return max;
    }
    return value;
}

i32 main() {
    /* The demo still exits with 42, but now exercises locals,
       calls, arithmetic, if statements, and assignment. */
    const i32 base = 40;
    i32 bonus = add(1, 1);
    i32 result = clamp(base + bonus, 0, 100);
    return result;
}
