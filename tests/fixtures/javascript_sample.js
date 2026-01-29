// Test fixture for JavaScript syntax highlighting
// Should test: strings, keywords, comments

// String literals
const greeting = "Hello, world!";
const template = `Template literal with ${greeting}`;
const singleQuote = 'Single quoted string';

// Keywords - declarations
let variable = 42;
var oldStyle = true;
const constant = "immutable";

// Keywords - control flow
if (true) {
    console.log("if keyword");
} else {
    console.log("else keyword");
}

// Keywords - loops
for (let i = 0; i < 10; i++) {
    if (i === 5) break;
    if (i === 3) continue;
}

while (false) {
    console.log("never executed");
}

// Keywords - function
function greet(name) {
    return `Hello, ${name}!`;
}

// Keywords - async/await
async function fetchData() {
    try {
        const response = await fetch("https://api.example.com");
        return await response.json();
    } catch (error) {
        throw new Error("Failed to fetch");
    } finally {
        console.log("cleanup");
    }
}

// Keywords - class
class Person {
    constructor(name) {
        this.name = name;
    }

    static create(name) {
        return new Person(name);
    }
}

// Keywords - import/export
export default Person;
export { greet, fetchData };

// Keywords - other
typeof greeting;
instanceof Person;
delete obj.prop;
void 0;
