FROM python:3.10-alpine
WORKDIR /code
#RUN apk add --no-cache gcc musl-dev linux-headers
COPY requirements.txt requirements.txt
RUN pip install -r requirements.txt
EXPOSE 8000
COPY . .
CMD ["uvicorn", "main:app", "--port", "8000", "--host" ,"0.0.0.0"]