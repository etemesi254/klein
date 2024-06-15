#!/bin/bash
j=1
for i in {1..100}
do
    start="$j day ago";
    end="${i} day ago";
    j=$((j+1));
    start_date="date -d '${start}' '+%Y-%m-%d'";
    end_date="date -d '${end}' '+%Y-%m-%d'";
    ss=$(eval "$start_date");
    ee=$(eval "$end_date");
    #echo  $ee,$ss,$start_date,$end_date

    curl "http://localhost:5001/neo?start_date=$ss&&end_date=$ee" >> /dev/null
    sleep 5
done
