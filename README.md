# EpidemicSimulator

This is a project to discover the feasibility of using Rust for large scale agent base modelling of Epidemics, and to
discover the impact that real world census data can have on the simulation.

It is inspired by EpiRust (https://github.com/thoughtworks/epirust) and similar projects.

It utilises UK Census data from 2011, to model the population of the UK and their movements. As well as open street
maps, to build an abstracted model of the real world

## V1-V1.7

These branches are part of the initial development phase, for my Dissertation. Key Features are:
A reasonably accurate simulator capable of predicting the 6th month period from the 14/08/2020 to 14/03/202 in York,
England. Utilising census data, to build a population with age, occupation and gender characteristics Uses OSM mapping
data to represent homes, schools and workplaces Unique schedules for each Citizen, where they travel between work and
home, sometimes using public transport Can simulate 3.5 million Citizens in just over an hour for 5000 timesteps (~7
months) on a workstation PC Scalability using multithreading Configurable Interventions to reduce the spread of the
disease

## Goals for Future Iterations

* Expand the complexity of a Citizens schedule to account for things like shopping, social events and weekends.
* Horizontal Scaling using MPI or similar
* Improve user experience and configuration of the disease
* Add dynamic disease risks, dependent on occupation
* Improve support for disease intervention techniques (mask wearing, lockdowns, vaccinations, etc)
* Better visualisation support for summaries
*

## V2

This is an extension of the Dissertation, to make the simulator more user-friendly and implement additional features.
Hopefully implementing some of the aforementioned goals

External Contributors are welcome, and new ideas/thoughts are much appreciated.

