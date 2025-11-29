// Fibonacci iterativo até 35
let n = 35;
let a = 0;
let b = 1;

for (let i = 0; i < n; i++) {
    let temp = a + b;
    a = b;
    b = temp;
}

console.log(b);
