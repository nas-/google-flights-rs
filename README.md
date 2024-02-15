# google-flights-rs
Unofficial API for google flights, impemented in Rust.


# Limitations
 The frontend uses the header x-googl-batchexecute-bgr to communicate with the backend.
 This parameter created deep in the JS files, which are deeply mangled with Google Closure compiler, and quite hard to follow, even while using a debugger.
 The parameter is dependent on the current time and the post request payload length, among other things.
 Missing this, the responses from the backend are not fully accurate.
 Contributions for the algorythm to calculate this parameter are welcome.



# Use
## Request for a single itinerary & offers

See [Flights Example](examples/flights.rs)


## Request Flight Graph

See [Graph Example](examples/graph.rs)



