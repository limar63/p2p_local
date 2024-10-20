# p2p_local
My implementation of a barebones gossiping app for usage on a single machine

To run the program, download the repo, open multiple terminal tabs, and write cargo run in these tabs. 
Here is an example of a command to run a node

./peer --period=5 --port=8080

You need to choose the available port and rerun the program if chosen the wrong one, the period number are seconds of how often current node will be gossiping connected nodes. The gossiping message is hard coded, you can find it by searching MESSAGE.

When you have an already running node - you can create the next ones in another tab by giving an extended run command

./peer --period=6 --port=8081 --connect="127.0.0.1:8080"

127.0.0.1:8080 is the node you would connect to (the first node).

Connect each tab individually, each with an available port. Parameters "port" and "period" are not allowed to be 0.
