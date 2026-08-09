FROM node:20-slim
WORKDIR /app
RUN groupadd -r app && useradd -r -g app app
COPY package*.json ./
RUN npm ci
COPY . .
RUN chown -R app:app /app
USER app
EXPOSE 8000
CMD ["npm", "run", "start"]
