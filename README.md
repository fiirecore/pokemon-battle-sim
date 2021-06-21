# Online Pokemon Battle Engine

## Installation

1. Install Rust from https://www.rust-lang.org/learn/get-started
2. Clone the repository
3. Run ```cargo build --all``` in the repository folder 
4. Executables will be in the target\debug directory

## Usage: 

The program uses TCP and defaults to port 28528

1. Open the server
2. Open two clients (the screen will be black on startup, this is normal)
3. Type the server's ip address into both clients.
4. If you cannot type and the screen is black with no "Connecting..." message, you have connected.
5. When both clients connect, the battle starts.

## Other:

See main code for the game here: https://github.com/DoNotDoughnut/firecore-game,
This repository is built on top of it.

Many moves are not implemented because I'm pretty sure I need to do them all manually
Also many move animations are also not in the game yet.
