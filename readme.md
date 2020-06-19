# What is rogiso

Rogiso is a module for prototype based object system with features below:

* Prototype based object system just like ECMAScript (but not the same)
* Essential concurrent lock to make the ability of multithreading
* Garbage collector to make the memory management easy
* Implemented via pure rust language to make the runtime safe
* High-level design for language and engine backend

The module is under heavy **development and test**, currently do not use 
it in your production level environment and design

# What are not the jobs of rogiso

The purpose of rogiso module is to make such a pure object system, without 
touching any language features and runtime environment. So the module has 
no implementation below:

* Language and grammar design
* Engine API design 
* Builtin standard library design
* Callable concept and implementation (such as function, method, and etc.)
* Virtual machine implementation
* Script loader, parser, compiler or any interpretter implementation

However, we provided the ability to integrate with the features above into
a full feature runtime engine

# Memory usage

According to the design of the system, currently a slotted object will take
about 256 bytes (56 bytes in page, rest in distributed heap)

* 64 bytes for base cost of a hash map
* 64 bytes for record info (such as prototype, flags, traps, injection)
* 72 bytes for more properties if needed (not essential, and dynamic)
* 32 bytes for garbage collection records
* 24 bytes for rw lock

Generally speaking, 256 bytes can store 32 pointers, or 64 integers, and 
I don't think the implementation wasted a lot of memory for a pure dynamic 
object system, which means a concurrent hash map is essential. Plus the 
data of flags, injection and garbage collection, 184 bytes will be 
occupied as a result.

So 4 GiB memory can take about 16M complex objects at most, and we will 
try to shrink the memory usage in future. 

Values below are not regarded as complex objects:

* undefined
* null
* boolean
* integer
* float
* symbol
* text (planning)

# Value

We use a 64-bit data to represent any possible value in the object 
system, which is widely used by many ECMAScript implementation.

According to the IEEE-754 specification, there are lots of `NaN`s in the 
64-bit float ranges. We just use a special predefined `NaN` as the real 
`NaN` in the value system, which make us to be able to regard the rest 
`NaN`s as various values with different types within just a 64-bit data.

A complex object will regard the value as something pointer alike, but
the pointer is temporary which means it will be invalid after a period.

# Symbol

We introduced symbol and symbol scope to extend traditional property key
system. Usually, text key is widely used today, and ECMAScript has another
kind of key, symbol. 

After researching, we decided to extend the symbol concept. There are two
parts in our symbol, one is symbol scope, another could be a value or a 
text. Within the new design:

* We can make a public symbol scope and all 
  text symbols in it could be regarded as traditional text keys. 
* We can make a symbol scope targeting script file path and all 
  symbols in it could be regarded as private keys shared in that file. 
* We can make a shared scope targeting multiple script files for friend 
  keys visibility
* We can create a private scope targeting to a single class, or piece of 
  codes as real private key

So the symbol system is more hackable for scripting system. We use a 
32-bit ID representing a symbol to simplify the hash map and acceleration. 
The symbol info could be resolved via API in `Isolate`.

# Garbage collection

A collector is implemented to collect garbages with a STW design. We are
still programming a concurrent collector to make the entire system more
elegant.

Usually, a value is protected via wrapping it into a `Pinned`. However, a
complex object is better to wrap into a `Local` to operate its properties
on stack. `Persistent` is needed if you'd like make it as a long-term 
reference in heap. `Weak` is such a wrapper to monitor the drop of a 
complex object.

A page-based memory description table is designed to locate all the 
complex object. The pointer-like part of a value usually means the info
to find the record on the description table. So the value may not be 
valid if the redirection records dropped after a memory refragment. 

We stored lock and GC info of the complex object on a page to keep the 
real data of the complex object no move during refragement.

# Integration

We have designed 5 traits for integration of the isolate system which 
can make you inject and keep value related information and behaviors.

* `InternalSlot` is such a trait which the system don't care what it is, 
  you can bind your self-implemented internal slot on any complex object
  to keep extra info
* `PropertyTrap` helps you to monitor a specified property reading and 
  writing on any complex object
* `SlotTrap` can interrupt any operation on a specfied complex object
* `TrapInfo` is designed as the information container while trap happen
* `Context` makes you have the ability to change the isolate behavior

# Optimization

A field shortcut support is implemented for simple field property trap
with template and version configuration, if you need JIT.

# TODOs, Timeline and Roadmap

## Finished Jobs (usually tested with codes in source file)

* [-] Implementation of prototype based object
* [-] Injection of property getter and setter
* [-] Injection of object operation
* [-] Support of object extra info
* [-] Basic supports for List object
* [-] Basic supports for Text object
* [-] Basic supports for Tuple object
* [-] Implementation of page based memory table and region
* [-] Implementation of value operation
* [-] Implementation of root supports
* [-] Implementation of field shortcuts
* [-] Implementation of reference map (remember set)

## Testing Jobs (implemented without test)

* [ ] Test of collector
* [ ] Concurrent object operation

## Developing Jobs

* [ ] Make prototype field direct stored in record
* [ ] Make color based region, not isolated based
* [ ] Export the control of optimization data to Context
* [ ] Add more API for list and text
* [ ] Make the collect of memory concurrent

## Future Jobs

* [ ] Optimize the text implementation
* [ ] Optimize the list implementation
* [ ] Optimize the lock (to keep the isolate concurrent, too many locks added)
* [ ] Optimize the arc (to make the rust compiler work, too many arc added)
* [ ] Optimize the memory operation
* [ ] Optimize the collector
* [ ] Optimize the record of complex object
* [ ] Full test for the isolate
* [ ] Integrate with the self-designed rogic language engine 
