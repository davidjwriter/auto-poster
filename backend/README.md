# Welcome to the X and Deso autoposter

This is a blank project for CDK development with TypeScript.

The `cdk.json` file tells the CDK Toolkit how to execute your app.

## Useful commands

- `npm run build` compile typescript to js
- `npm run watch` watch for changes and compile
- `npm run test` perform the jest unit tests
- `cdk deploy` deploy this stack to your default AWS account/region
- `cdk diff` compare deployed stack with current state
- `cdk synth` emits the synthesized CloudFormation template

# Design

- DynamoDB Table: to store 24 hours worth of posts
- AddToDB Lambda: adds a post to the database
- GeneratePost Lambda: uses OpenAI and our current Substack posts to generate 24\*6 different unique posts, store them in DB, runs once every week
- PostToDeso Lambda: function that subscribes to an SNS topic, posts the post to Deso
- PostToX Lambda: function that subscribes to an SNS topic, posts the post to X
- SendPost Lambda: function that runs every hour, takes a post from DB, sends to the SNS topic, then deletes post from DB

# Prompt

The prompt we'll be using to generate posts is as follows:

Create 36 powerful short Tweets that inspire conversation from this article. Respond with the Tweets in JSON format like this: {"tweets": ["post": <tweet>, "type": <POST or THREAD>]}

## Requirements

- 1 DynamoDB Table
- 1 SNS
- 1 Lambda function API
- 2 Lambda functions subscribers
- 2 Lambda functions events
