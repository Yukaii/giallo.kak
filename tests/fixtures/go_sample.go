// Test fixture for Go syntax highlighting
// Should test: strings, keywords, comments

package main

import (
	"fmt"
	"strings"
)

// String literals
var greeting = "Hello, world!"
var multiline = `This is a
multiline raw string`
var rune_lit = 'a'

// Keywords - control flow
func main() {
	if true {
		fmt.Println("if keyword")
	} else {
		fmt.Println("else keyword")
	}

	// Keywords - loops
	for i := 0; i < 10; i++ {
		if i == 5 {
			break
		}
		if i == 3 {
			continue
		}
	}

	// Keywords - switch
	switch x := 42; x {
	case 0:
		fmt.Println("zero")
	case 42:
		fmt.Println("answer")
	default:
		fmt.Println("other")
	}

	// Keywords - defer/go
	defer cleanup()
	go asyncWork()

	// Keywords - select (for channels)
	ch := make(chan int)
	select {
	case msg := <-ch:
		fmt.Println(msg)
	default:
		fmt.Println("no message")
	}
}

// Keywords - function and return
func greet(name string) string {
	return fmt.Sprintf("Hello, %s!", name)
}

// Keywords - struct and interface
type Person struct {
	Name string
	Age  int
}

type Greeter interface {
	Greet() string
}

// Keywords - method
func (p Person) Greet() string {
	return "Hello from " + p.Name
}

// Keywords - const and var
const MaxSize = 100
var GlobalVar int

// Keywords - type definition
type StringSlice []string

// Keywords - map
var cache = map[string]int{
	"key": 42,
}

// Keywords - range
func iterate(items []string) {
	for idx, item := range items {
		fmt.Printf("%d: %s\n", idx, item)
	}
}
