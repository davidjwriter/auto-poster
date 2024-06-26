import { Runtime, Function, Code, CfnLayerVersion } from 'aws-cdk-lib/aws-lambda';
import * as sns from 'aws-cdk-lib/aws-sns';
import * as sqs from 'aws-cdk-lib/aws-sqs';
import { App, Stack, RemovalPolicy, aws_sns_subscriptions, aws_lambda_event_sources } from 'aws-cdk-lib';
import { Rule, Schedule } from 'aws-cdk-lib/aws-events';
import { LambdaFunction } from 'aws-cdk-lib/aws-events-targets';
import { RetentionDays } from 'aws-cdk-lib/aws-logs';
import * as s3 from 'aws-cdk-lib/aws-s3';
import { IResource, LambdaIntegration, MockIntegration, PassthroughBehavior, RestApi, Cors } from 'aws-cdk-lib/aws-apigateway';
import { AttributeType, Table } from 'aws-cdk-lib/aws-dynamodb';
import { Duration, DockerImage } from 'aws-cdk-lib';
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
    const desoUser = process.env.DESO_USER || "No Deso User";
    const desoPrivateKey = process.env.DESO_PRIVATE_KEY || "No Deso Private Key";
    const consumerKey = process.env.CONSUMER_KEY || 'NO Twitter Consumer Key';
    const consumerSecret = process.env.CONSUMER_SECRET || 'NO Twitter Consumer Secret';
    const accessToken = process.env.ACCESS_TOKEN || 'NO Twitter Access Key';
    const accessTokenSecret = process.env.ACCESS_TOKEN_SECRET || 'NO Twitter Access Key Secret';
    const scheduledPosts = "ScheduledPosts";

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

    const scheduledTable = new Table(this, 'ScheduledPosts', {
      partitionKey: {
        name: 'uuid',
        type: AttributeType.STRING
      },
      readCapacity: 1,
      writeCapacity: 1,
      tableName: scheduledPosts,
      removalPolicy: RemovalPolicy.RETAIN, // NOT recommended for production code
    });

    // Create an IAM role for the Lambda function
    const lambdaRole = new iam.Role(this, 'LambdaRole', {
      assumedBy: new iam.ServicePrincipal('lambda.amazonaws.com'),
    });

    // Attach the basic Lambda execution policy (You can adjust permissions as needed)
    lambdaRole.addManagedPolicy(iam.ManagedPolicy.fromAwsManagedPolicyName('service-role/AWSLambdaBasicExecutionRole'));
  
    // Create an SNS topic and subscribe the postToTwitter and postToDeso lambdas

    // Create an SQS FIFO Queue
    const desoQueue = new sqs.Queue(this, 'DesoQueue', {
      fifo: true,
      contentBasedDeduplication: true,
      queueName: 'DesoQueue.fifo',
      visibilityTimeout: Duration.seconds(300),
      deadLetterQueue: {
        maxReceiveCount: 1, // Move to DLQ after 1 failed attempt
        queue: new sqs.Queue(this, 'DeadLetterQueue', {
          fifo: true,
          queueName: 'DeadLetterQueue.fifo',
        }),
      },
    });

    const xQueue = new sqs.Queue(this, 'XQueue', {
      fifo: true,
      contentBasedDeduplication: true,
      queueName: 'XQueue.fifo',
      visibilityTimeout: Duration.seconds(300)
    });

    const postTopic = new sns.Topic(this, 'NewPostTopic', {
      topicName: 'NewPostTopic.fifo',
      fifo: true,
      contentBasedDeduplication: true,
    });

    // Create an API Gateway resource for each of the CRUD operations
    const api = new RestApi(this, 'PostAPI', {
      restApiName: 'Post API',
      defaultCorsPreflightOptions: {
        allowOrigins: Cors.ALL_ORIGINS,
        allowMethods: Cors.ALL_METHODS,
        allowHeaders: Cors.DEFAULT_HEADERS,
      }
    });
    const generateAPI = new RestApi(this, 'GenerateAPI', {
      restApiName: 'Generate API',
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
        SCHEDULED_TABLE_NAME: scheduledPosts,
        SNS_ARN: postTopic.topicArn
      },
      logRetention: RetentionDays.ONE_WEEK,
      role: lambdaRole
    });

    postTopic.grantPublish(sendPosts);
    dynamoTable.grantReadWriteData(sendPosts);
    scheduledTable.grantReadWriteData(sendPosts);
    const postEvent = new Rule(this, 'postEvent', {
      schedule: Schedule.expression('rate(1 hour)'),
    });
    postEvent.addTarget(new LambdaFunction(sendPosts));

    // Get Posts
    const getPosts = new Function(this, 'getPosts', {
      description: "Get all posts in DB",
      code: Code.fromAsset('lib/lambdas/getPosts/target/x86_64-unknown-linux-musl/release/lambda'),
      runtime: Runtime.PROVIDED_AL2,
      handler: 'not.required',
      environment: {
        RUST_BACKTRACE: '1',
        TABLE_NAME: 'Posts',
      },
      logRetention: RetentionDays.ONE_WEEK,
      role: lambdaRole
    });
    dynamoTable.grantReadWriteData(getPosts);

    // Edit Posts
    const editPost = new Function(this, 'editPost', {
      description: "Edit posts to the DB",
      code: Code.fromAsset('lib/lambdas/editPost/target/x86_64-unknown-linux-musl/release/lambda'),
      runtime: Runtime.PROVIDED_AL2,
      handler: 'not.required',
      environment: {
        RUST_BACKTRACE: '1',
        TABLE_NAME: 'Posts'
      },
      logRetention: RetentionDays.ONE_WEEK,
      role: lambdaRole
    });
    dynamoTable.grantWriteData(editPost);

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

    const addScheduledPost = new Function(this, 'addScheduledPost', {
      description: "Add new scheduled posts to the DB",
      code: Code.fromAsset('lib/lambdas/addScheduledPost/target/x86_64-unknown-linux-musl/release/lambda'),
      runtime: Runtime.PROVIDED_AL2,
      handler: 'not.required',
      environment: {
        RUST_BACKTRACE: '1',
        TABLE_NAME: scheduledPosts
      },
      logRetention: RetentionDays.ONE_WEEK,
      role: lambdaRole
    });

    scheduledTable.grantWriteData(addScheduledPost);

    // Integrate lambda functions with an API gateway
    const addPostAPI = new LambdaIntegration(addPost);
    const addScheduledPostAPI = new LambdaIntegration(addScheduledPost);
    const getPostsAPI = new LambdaIntegration(getPosts);
    const editPostAPI = new LambdaIntegration(editPost);

    const add = api.root.addResource('add');
    add.addMethod('POST', addPostAPI);

    const addSchedule = api.root.addResource('addSchedule');
    addSchedule.addMethod('POST', addScheduledPostAPI);

    const get = api.root.addResource('getPosts');
    get.addMethod('GET', getPostsAPI);

    const edit = api.root.addResource('editPosts');
    edit.addMethod('POST', editPostAPI);

    lambdaRole.addToPolicy(new iam.PolicyStatement({
      actions: ['execute-api:Invoke'],
      resources: [api.arnForExecuteApi()],  // Restrict to your API Gateway resource
    }));
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
      schedule: Schedule.rate(Duration.days(1)),
    });
    generateEvent.addTarget(new LambdaFunction(generatePosts));

    // Add api endpoint for generation
    const generateAPIIntegration = new LambdaIntegration(generatePosts);
    const generate = generateAPI.root.addResource('generate');
    generate.addMethod('POST', generateAPIIntegration);

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
        CONSUMER_KEY: consumerKey,
        CONSUMER_SECRET: consumerSecret,
        ACCESS_TOKEN: accessToken,
        ACCESS_TOKEN_SECRET: accessTokenSecret
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
      timeout: Duration.minutes(5),
      environment: {
        RUST_BACKTRACE: '1',
        SNS_ARN: postTopic.topicArn,
        DESO_USER: desoUser,
        DESO_PRIVATE_KEY: desoPrivateKey
      },
      logRetention: RetentionDays.ONE_WEEK,
      role: lambdaRole
    });

    // Add lambda event source as the queue and add each queue as a subscriber
    // This will prevent any duplicate messages being passed
    desoQueue.grantConsumeMessages(postToDeso);
    xQueue.grantConsumeMessages(postToTwitter);
    postToDeso.addEventSource(new aws_lambda_event_sources.SqsEventSource(desoQueue));
    postToTwitter.addEventSource(new aws_lambda_event_sources.SqsEventSource(xQueue));
    postTopic.addSubscription(new aws_sns_subscriptions.SqsSubscription(desoQueue));
    postTopic.addSubscription(new aws_sns_subscriptions.SqsSubscription(xQueue));
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
