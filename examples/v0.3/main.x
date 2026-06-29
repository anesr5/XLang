module v0_3_demo

struct Vec2 {
    i32 x;
    i32 y;
}

i32 sum_vec2() {
    Vec2 p = { 3, 4 };
    i32 total = p.x + p.y;
    p.y = 10;
    return total;
}

i32 main() {
    return sum_vec2();
}
