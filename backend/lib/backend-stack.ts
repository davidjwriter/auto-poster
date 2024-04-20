import { Runtime, Function, Code, CfnLayerVersion } from 'aws-cdk-lib/aws-lambda';
import * as sns from 'aws-cdk-lib/aws-sns';
import { App, Stack, RemovalPolicy } from 'aws-cdk-lib';
import { Rule, Schedule } from 'aws-cdk-lib/aws-events';
import { LambdaFunction } from 'aws-cdk-lib/aws-events-targets';
import { RetentionDays } from 'aws-cdk-lib/aws-logs';
import * as s3 from 'aws-cdk-lib/aws-s3';
import { IResource, LambdaIntegration, MockIntegration, PassthroughBehavior, RestApi, Cors } from 'aws-cdk-lib/aws-apigateway';
import { AttributeType, Table } from 'aws-cdk-lib/aws-dynamodb';
import { Duration, DockerImage } from 'aws-cdk-lib';
import path = require('path');
import * as iam from 'aws-cdk-lib/aws-iam';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import { LambdaSubscription } from 'aws-cdk-lib/aws-sns-subscriptions';
import { BlockPublicAccess, BucketAccessControl } from 'aws-cdk-lib/aws-s3';
import { NodejsFunction, NodejsFunctionProps } from 'aws-cdk-lib/aws-lambda-nodejs';
import { AwsIntegration } from 'aws-cdk-lib/aws-apigateway';

export class AutoPosterStack extends Stack {
  constructor(app: App, id: string) {
    super(app, id);
    const openAiApiKey = process.env.OPEN_AI_API_KEY || 'NO_API_KEY';
    const twitterKey = process.env.TWITTER_API_KEY || 'NO Twitter API Key';
    const twitterSecret = process.env.TWITTER_API_SECRET || 'No Twitter Secret';
    const desoUser = process.env.DESO_USER || "No Deso User";
    const desoPrivateKey = process.env.DESO_PRIVATE_KEY || "No Deso Private Key";

    // Setup our dynamo db table
    const dynamoTable = new Table(this, 'Posts', {
      partitionKey: {
        name: 'uuid',
        type: AttributeType.STRING
      },
      readCapacity: 1,
      writeCapacity: 1,
      tableName: 'Posts',

      /**
       *  The default removal policy is RETAIN, which means that cdk destroy will not attempt to delete
       * the new table, and it will remain in your account until manually deleted. By setting the policy to
       * DESTROY, cdk destroy will delete the table (even if it has data in it)
       */
      removalPolicy: RemovalPolicy.RETAIN, // NOT recommended for production code
    });

    // Create an IAM role for the Lambda function
    const lambdaRole = new iam.Role(this, 'LambdaRole', {
      assumedBy: new iam.ServicePrincipal('lambda.amazonaws.com'),
    });

    // Attach the basic Lambda execution policy (You can adjust permissions as needed)
    lambdaRole.addManagedPolicy(iam.ManagedPolicy.fromAwsManagedPolicyName('service-role/AWSLambdaBasicExecutionRole'));
  
    // Create an SNS topic and subscribe the postToTwitter and postToDeso lambdas
    const postTopic = new sns.Topic(this, 'NewPostTopic');

    // Create an API Gateway resource for each of the CRUD operations
    const api = new RestApi(this, 'PostAPI', {
      restApiName: 'Post API',
      defaultCorsPreflightOptions: {
        allowOrigins: Cors.ALL_ORIGINS,
        allowMethods: Cors.ALL_METHODS,
        allowHeaders: Cors.DEFAULT_HEADERS,
      }
    });

    const sendPosts = new Function(this, 'sendPosts', {
      description: "Takes a post to send to SNS topic from DB",
      code: Code.fromAsset('lib/lambdas/sendPosts/target/x86_64-unknown-linux-musl/release/lambda'),
      runtime: Runtime.PROVIDED_AL2,
      handler: 'not.required',
      timeout: Duration.minutes(5),
      environment: {
        RUST_BACKTRACE: '1',
        TABLE_NAME: 'Posts',
        SNS_ARN: postTopic.topicArn
      },
      logRetention: RetentionDays.ONE_WEEK,
      role: lambdaRole
    });

    postTopic.grantPublish(sendPosts);
    dynamoTable.grantReadWriteData(sendPosts);
    const postEvent = new Rule(this, 'postEvent', {
      schedule: Schedule.expression('rate(1 hour)'),
    });
    postEvent.addTarget(new LambdaFunction(sendPosts));

    // 1 Lambda function API
    const addPost = new Function(this, 'addPost', {
      description: "Add new posts to the DB",
      code: Code.fromAsset('lib/lambdas/addPost/target/x86_64-unknown-linux-musl/release/lambda'),
      runtime: Runtime.PROVIDED_AL2,
      handler: 'not.required',
      timeout: Duration.minutes(5),
      environment: {
        RUST_BACKTRACE: '1',
        TABLE_NAME: 'Posts'
      },
      logRetention: RetentionDays.ONE_WEEK,
      role: lambdaRole
    });

    dynamoTable.grantWriteData(addPost);

    // Integrate lambda functions with an API gateway
    const addPostAPI = new LambdaIntegration(addPost);

    const add = api.root.addResource('add');
    add.addMethod('POST', addPostAPI);

    // Event triggered Lambdas: generatePosts and sendPosts
    const generatePosts = new Function(this, 'generatePosts', {
      description: "Generates new posts and calls addPost API",
      code: Code.fromAsset('lib/lambdas/generatePosts/target/x86_64-unknown-linux-musl/release/lambda'),
      runtime: Runtime.PROVIDED_AL2,
      handler: 'not.required',
      timeout: Duration.minutes(5),
      environment: {
        RUST_BACKTRACE: '1',
        OPEN_AI_API_KEY: openAiApiKey,
        ADD_TO_DB_API: api.url
      },
      logRetention: RetentionDays.ONE_WEEK,
      role: lambdaRole
    });
    const generateEvent = new Rule(this, 'generateEvent', {
      schedule: Schedule.expression('rate(1 week)'),
    });
    generateEvent.addTarget(new LambdaFunction(generatePosts));

    // 2 Lambda function subscribers

    // Post to Twitter
    const postToTwitter = new Function(this, 'postToTwitter', {
      description: "posts the post to X",
      code: Code.fromAsset('lib/lambdas/postToTwitter/target/x86_64-unknown-linux-musl/release/lambda'),
      runtime: Runtime.PROVIDED_AL2,
      handler: 'not.required',
      timeout: Duration.minutes(5),
      environment: {
        RUST_BACKTRACE: '1',
        SNS_ARN: postTopic.topicArn,
        TWITTER_API_KEY: twitterKey,
        TWITTER_SECRET: twitterSecret
      },
      logRetention: RetentionDays.ONE_WEEK,
      role: lambdaRole
    });

    // Post to Deso
    const postToDeso = new Function(this, 'postToDeso', {
      description: "posts the post to deso",
      code: Code.fromAsset('lib/lambdas/postToDeso/target/x86_64-unknown-linux-musl/release/lambda'),
      runtime: Runtime.PROVIDED_AL2,
      handler: 'not.required',
      environment: {
        RUST_BACKTRACE: '1',
        SNS_ARN: postTopic.topicArn,
        DESO_USER: desoUser,
        DESO_PRIVATE_KEY: desoPrivateKey
      },
      logRetention: RetentionDays.ONE_WEEK,
      role: lambdaRole
    });

    postTopic.addSubscription(new LambdaSubscription(postToTwitter));
    postTopic.addSubscription(new LambdaSubscription(postToDeso));

  }
}

export function addCorsOptions(apiResource: IResource, httpMethod: string) {
  apiResource.addMethod(httpMethod, new MockIntegration({
    integrationResponses: [{
      statusCode: '200',
      responseParameters: {
        'method.response.header.Access-Control-Allow-Headers': "'Content-Type,X-Amz-Date,Authorization,X-Api-Key,X-Amz-Security-Token,X-Amz-User-Agent'",
        'method.response.header.Access-Control-Allow-Origin': "'*'",
        'method.response.header.Access-Control-Allow-Credentials': "'false'",
        'method.response.header.Access-Control-Allow-Methods': "'OPTIONS,GET,PUT,POST,DELETE'",
      },
    }],
    passthroughBehavior: PassthroughBehavior.NEVER,
    requestTemplates: {
      "application/json": "{\"statusCode\": 200}"
    },
  }), {
    methodResponses: [{
      statusCode: '200',
      responseParameters: {
        'method.response.header.Access-Control-Allow-Headers': true,
        'method.response.header.Access-Control-Allow-Methods': true,
        'method.response.header.Access-Control-Allow-Credentials': true,
        'method.response.header.Access-Control-Allow-Origin': true,
      },
    }]
  });
}

const app = new App();
new AutoPosterStack(app, 'AutoPosterStack');
app.synth();
