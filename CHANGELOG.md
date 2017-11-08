## 0.1.0 - First Release
* Added the compiler, with all the basic features
* Added the corresponding syntax definitions

## 0.2.0 - LLVM Integration
* The compiler now generates LLVM IR, executed with the MCJIT engine
* The following features were added to the language:
    * Branches: The usual `if` / `else` construct
    * Loops: The `while` keyword can be used to control a loop more precisely
      than the `for`-`in` iterators
    * Variables: It's now possible to change the value of an existing
      `let`-binding
    * Property rewriting: It's now possible to assign values to the properties
      of entities, including virtual "sub-properties" (such as `.pitch` for the
      `angles` property)
    * A library of common functions:
        * `create(name: String, class: String) -> Entity`
        * `clone(ent: Entity) -> Entity`
        * `remove(ent: Entity)`
        * `find(name: String) -> Entity`
        * `find_class(class: String) -> [Entity]`
        * `exp(val: f64) -> f64`
        * `sqrt(val: f64) -> f64`
        * `pow(val: f64, exp: f64) -> f64`
        * `sin(val: f64) -> f64`
        * `cos(val: f64) -> f64`
        * `floor(val: f64) -> f64`
        * `ceil(val: f64) -> f64`
        * `round(val: f64) -> f64`
        * `length(x: f64, y: f64, z: f64) -> f64`
        * `rand(low: f64, high: f64) -> f64`
        * `range(from: f64, to: f64) -> [f64]`
        * `to_string(val: f64) -> String`
        * `parse(val: String) -> f64`
