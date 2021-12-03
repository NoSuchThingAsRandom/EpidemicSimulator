# EpidemicSimulator

This is a project to discover the feasibility of using Rust for large scale agent base modelling of Epidemics, and to
discover the impact that real world census data can have on the simulation.

It is inspired by EpiRust (https://github.com/thoughtworks/epirust) and similar projects.

It utilises UK Census data from 2011, to model the population of the UK and their movements.

## V1

This is a first initial baseline version, that can load in a population from Census Files.

It also allocates workplaces based on Occupations Types and in different OutputAreas depending on the Home Residence to
Workpalce position Table (https://www.nomisweb.co.uk/census/2011/wf01bew)

## TODO In Future Iterations

* Implement Multithreading
* Add dynamic disease risks, dependent on occupation
* Add support for disease intervention techniques (mask wearing, lockdowns, vaccinations, etc)
* Better visualisation support for summaries

## Current Problem

Trying to allow spread of disease (increase the networks of agents), as seems to get stuck in a cluster

Measures taken:

1. Randomly allocating households
2. Assigning workplaces inside the same output area
3. Assigning workplaces outside the output area