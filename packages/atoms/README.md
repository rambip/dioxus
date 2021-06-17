<div align="center">
  <h1>⚛ Dirac</h1>
  <p>
    <strong>Global state for Dioxus</strong>
  </p>
</div>

Dirac is a global state management toolkit for Dioxus, built on the concept of composable atoms of state:

```rust
const COUNT: Atom<u32> = |_| 0;

const Incr: FC<()> = |cx| {
    let (count, set_count) = dirac::use_read_write(&cx, &COUNT);
    rsx!(in cx, button { onclick: move |_| set_count(count + 1), "+" } )
}

const Decr: FC<()> = |cx| {
    let (count, set_count) = dirac::use_read_write(&cx, &COUNT);
    rsx!(in cx, button { onclick: move |_| set_count(count - 1), "-" } )
}

const App: FC<()> = |cx| {
    let count = dirac::use_read(&cx, &COUNT);
    cx.render(rsx!{
        "Count is {count}"
        Incr {}
        Decr {}
    })
}
```

Dirac provides a global state management API for Dioxus apps built on the concept of "atomic state." Instead of grouping state together into a single bundle like Redux, Dyon provides individual building blocks of state called atoms. These atoms can be set/get anywhere in the app and combined to craft complex state. Dyon should be easier to learn, more efficient than Redux, and allow you to build complex Dioxus apps with relative ease.

## Support

Dyon is officially supported by the Dioxus team. By doing so, we are "planting our flag in the sand" for atomic state management instead of bundled (Redux-style) state management. Atomic state management fits well with the internals of Dioxus and idiomatic Rust, meaning Dirac state management will be faster, more efficient, and less sensitive to data races than Redux-style apps.

Internally, Dioxus uses batching to speed up linear-style operations. Dirac integrates with this batching optimization, making app-wide state changes extremely fast. This way, Dirac can be pushed significantly harder than Redux without the need to enable/disable debug flags to prevent performance slowdowns.

# Guide

## Atoms

A simple atom of state is defined globally as a const:

```rust
const LightColor: Atom<&str> = |_| "Green";
```

This atom of state is initialized with a value of `"Green"`. The atom that is returned does not actually contain any values. Instead, the atom's key - which is automatically generated in this instance - is used in the context of a Dioxus App.

This is then later used in components like so:

```rust
fn App(ctx: Context<()>) -> VNode {
    // The Atom root must be initialized at the top of the application before any use_Atom hooks
    dirac::intialize_root(&ctx, |_| {});

    let color = dirac::use_read(&ctx, &LightColor);

    ctx.render(rsx!( h1 {"Color of light: {color}"} ))
}
```

Atoms are considered "Writable" objects since any consumer may also set the Atom's value with their own:

```rust
fn App(ctx: Context<()>) -> VNode {
    let color = dirac::use_read(&ctx, &LightColor);
    let set_color = dirac::use_write(&ctx, &LightColor);
    ctx.render(rsx!(
        h1 { "Color: {color}" }
        button { onclick: move |_| set_color("red"), "red" }
        button { onclick: move |_| set_color("yellow"), "yellow" }
        button { onclick: move |_| set_color("green"), "green" }
    ))
}
```

"Reading" a value with use_read subscribes that component to any changes to that value while "Writing" a value does not. It's a good idea to use `write-only` whenever it makes sense to prevent unnecessary evaluations. Both `read` and `write` may be combined together to provide a `use_state` style hook.

```rust
fn App(ctx: Context<()>) -> VNode {
    let (color, set_color) = dirac::use_read_write(&ctx, &Light);
    ctx.render(rsx!(
        h1 { "Color: {color}" }
        button { onclick: move |_| set_color("red"), "red" }
    ))
}
```

## Selectors

Selectors are a concept popular in the JS world as a way of narrowing down a selection of state outside the VDOM lifecycle. Selectors have two functions: 1 summarize/narrow down some complex state and 2) memoize calculations.

Selectors are only `readable` - they cannot be set. This differs from RecoilJS where selectors _are_ `writable`. Selectors, as you might've guessed, "select" some data from atoms and other selectors.

Selectors provide a `SelectorApi` which essentially exposes a read-only `AtomApi`. They have the `get` method which allows any readable valued to be obtained for the purpose of generating a new value. A `Selector` may only return `'static` data, however `SelectorBorrowed` may return borrowed data.

returning static data:

```rust
const Light: Atom<&'static str> = |_| "Green";
const NumChars: Selector<usize> = |api| api.get(&Light).len();
```

Selectors will update their selection and notify subscribed components whenever their dependencies also update. The `get` calls in a selector will subscribe that selector to whatever `Readable` is being `get`-ted. Unlike hooks, you may use `get` in conditionals; an internal algorithm decides when a selector needs to be updated based on what it `get`-ted last time it was ran.

Selectors may also returned borrowed data:

```rust
const Light: Atom<&'static str> = |_| "Green";
const ThreeChars: SelectorBorrowed<str> = |api| api.get(&Light).chars().take(3).unwrap();
```

Unfortunately, Rust tries to coerce `T` (the type in the Selector generics) to the `'static` lifetime because we defined it as a static. `SelectorBorrowed` defines a type for a function that returns `&T` and provides the right lifetime for `T`. If you don't like having to use a dedicated `Borrowed` type, regular functions work too:

```rust
// An atom
fn Light(api: &mut AtomBuilder) -> &'static str {
    "Green"
}

// A selector
fn ThreeChars(api: &mut SelectorApi) -> &'static str {
    api.get(&Light).chars().take(3).unwrap()
}
```

Returning borrowed data is generally frowned upon, but may be useful when used wisely.

- If a selected value equals its previous selection (via PartialEq), the old value must be kept around to avoid evaluating subscribed components.
- It's unlikely that a change in a dependency's data will not change the selector's output.

In general, borrowed selectors introduce a slight memory overhead as they need to retain previous state to safely memoize downstream subscribers. The amount of state kept around scales with the amount of `gets` in a selector - though as the number of dependencies increase, the less likely the selector actually stays memoized. Dirac tries to optimize this behavior the best it can to balance component evaluations with memory overhead.

## Families

You might notice that collections will not be performant with just sets/gets. We don't want to clone our entire HashMap every time we want to insert or remove an entry! That's where `Families` come in. Families are memoized collections (HashMap and OrdMap) that wrap the immutable datastructures library `im-rc`. Families are defined by a function that takes the FamilyApi and returns a function that provides a default value given a key. In this example, we insert a value into the collection when initialized, and then return a function that takes a key and returns a default value.

```rust
const CloudRatings: AtomFamily<&str, i32> = |api| {
    api.insert("Oracle", -1);
    |key| match key {
        "AWS" => 1,
        "Azure" => 2,
        "GCP" => 3,
        _ => 0
    }
}
```

Whenever you `select` on a `Family`, the ID of the entry is tracked. Other subscribers will only be updated if they too select the same family with the same key and that value is updated. Otherwise, these subscribers will never re-render on an "insert", "remove", or "update" of the collection. You could easily implement this yourself with Atoms, immutable datastructures, and selectors, but our implementation is more efficient and first-class.

```rust
fn App(ctx: Context<()>) -> VNode {
    let (rating, set_rating) = dirac::use_read_write(ctx, CloudRatings.select("AWS"));
    ctx.render(rsx!(
        h1 { "AWS rating: {rating}" }
        button { onclick: move |_| set_rating((rating + 1) % 5), "incr" }
    ))
}
```