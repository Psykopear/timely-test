# Timely-dataflow toy project

This is a toy project used to start working and understanding timely-dataflow.

The idea of this project is to build a dataflow that supports real time queries on continually updated data.
The data is a finite stream of information, but I want to start giving out results before the dataset is completely retrieved,
and also update filtered results when new data comes in.

The idea is to generate a list of applications installed on a Linux machine (`.desktop` files retrieved from several directories),
retrieve an appropriate icon file for each entry and allow the user to filter the entries by name or description with a fuzzy matching algorithm on the input.

# TODO
- Only walkdir once, and send each entry to a different worker.
  Right now every worker does the walkdir
- Allow interactive input of search query, then send the string to all workers.
- Keep the list of results in each worker, use it as first input for each subsequent query string
