// Conta quantos primos existem até 10000
let count = 0;

for (let n = 2; n <= 10000; n++) {
    let isPrime = true;
    let d = 2;

    while (d * d <= n) {
        if (n % d === 0) {
            isPrime = false;
            break;
        }
        d++;
    }

    if (isPrime) count++;
}

console.log(count);
