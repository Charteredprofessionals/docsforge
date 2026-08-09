FROM node:20-slim
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
EXPOSE 8000
CMD ["npm", "run", "start"]
