const keys = [
    'AIzaSyAGofcknbeT-x1cj7PYOaj-vwlzNkUiaBw',
    'AIzaSyAqJmwPeB8dZTHUtpuWDVGBsm1ihUjyH48',
    'AIzaSyDmVCJNF1SB1kPXdyt53Tf2zVu-9vyeHio'
];
const result = keys.map(k => {
    const obf = k.split('').map(c => c.charCodeAt(0) ^ 0x5A);
    return '&[' + obf.join(', ') + '],';
}).join('\n');
process.stdout.write(result);
