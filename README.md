# p2p_local
My implementation of a barebones gossiping app for usage on a single machine

To run the program, download the repo, open multiple terminal tabs, and write cargo run in these tabs. 
Here is an example of a command to run a node

./peer --period=5 --port=8080

You need to choose the available port and rerun the program if chosen the wrong one, the period number are seconds of how often current node will be gossiping connected nodes. The gossiping message is hard coded, you can find it by searching MESSAGE.

When you have an already running node - you can create the next ones in another tab by giving an extended run command

./peer --period=6 --port=8081 --connect="127.0.0.1:8080"

127.0.0.1:8080 is the node you would connect to (the first node).

Connect each tab individually, each with a port available.


Regards to how the project is structured - there is an initial loop for getting input from the console and then 2 separate async tasks are running - one for a server role of the role and another for a client role. 
Wanted to split functions for each role in 2 different files, but they are reusing some functions, and not reusing enough to create another file for "helper" methods so decided to keep them all in 1 big blob.
