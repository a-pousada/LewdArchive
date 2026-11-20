FROM python:3.12-slim
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1 MODEL_CACHE=/models WD_BIND_ADDRESS=0.0.0.0
WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt \
    && mkdir /models && chown 65532:65532 /models
COPY service.py .
USER 65532:65532
EXPOSE 5000
CMD ["python", "service.py"]
