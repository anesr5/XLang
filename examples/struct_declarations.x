module structs

struct Player {
    i32 hp;
    bool alive;
}

struct Score {
    i32 value;
}

i32 main() {
    // Struct declarations are parsed and validated, but values/layout are postponed.
    return 0;
}
