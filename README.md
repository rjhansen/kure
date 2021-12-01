# kure

I'm building a switching heating element for a fishtank, with the help of 
a Raspberry Pi, a smart socket, and some parts.  Although the DS18B20 
thermometer is really a neat piece of work it has some limitations, like
only producing data in an inefficient-to-access pure-ASCII form, not 
including timestamps on the data, and so on.

Kure, named for the last island in the Hawaiian archipelago, fixes this:
it's a well-behaved daemon that once a minute updates `/etc/ds18b20` with
properly-formatted JSON detailing all the thermometers attached to the
system, their current temperatures, and the time of recording.