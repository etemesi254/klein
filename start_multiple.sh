#!/bin/bash
for i in {1..5}
do
    curl "http://localhost:5001/neo?start_date=2023-02-10&&end_date=2023-02-12"  &
done
