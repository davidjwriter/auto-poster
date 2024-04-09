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
    // dotenv.config({ path: path.resolve(__dirname, '.env') });
    const openAiApiKey = process.env.OPEN_AI_API_KEY || 'NO_API_KEY';
    const privateKey = process.env.PRIVATE_KEY || 'NO_PRIVATE_KEY';
    const nftStoreApiKey = process.env.NFT_STORAGE_API_KEY || 'No NFT Store API Key';
    const cloudConvertApiKey = process.env.CLOUD_CONVERT_API_KEY || 'No Cloud Conver API Key';

    // Setup our dynamo db table
    const dynamoTable = new Table(this, 'Recipes', {
      partitionKey: {
        name: 'uuid',
        type: AttributeType.STRING
      },
      readCapacity: 1,
      writeCapacity: 1,
      tableName: 'Recipes',

      /**
       *  The default removal policy is RETAIN, which means that cdk destroy will not attempt to delete
       * the new table, and it will remain in your account until manually deleted. By setting the policy to
       * DESTROY, cdk destroy will delete the table (even if it has data in it)
       */
      removalPolicy: RemovalPolicy.RETAIN, // NOT recommended for production code
    });

    // User's dynamo db table
    const userTable = new Table(this, 'RecipeBooks', {
      partitionKey: {
        name: 'username',
        type: AttributeType.STRING
      },
      sortKey: {
        name: 'uuid',
        type: AttributeType.STRING
      },
      readCapacity: 1,
      writeCapacity: 1,
      tableName: 'RecipeBooks'
    });


    // Lambda function to add a new recipe
    // Expects a string URL
    const s3Bucket = new s3.Bucket(this, 'RecipeImages', {
      versioned: true,
      removalPolicy: RemovalPolicy.RETAIN,
      blockPublicAccess: BlockPublicAccess.BLOCK_ACLS,
      accessControl: BucketAccessControl.BUCKET_OWNER_FULL_CONTROL,
      publicReadAccess: true,
    });

    // Create an IAM role for the Lambda function
    const lambdaRole = new iam.Role(this, 'LambdaRole', {
      assumedBy: new iam.ServicePrincipal('lambda.amazonaws.com'),
    });

    // Attach the basic Lambda execution policy (You can adjust permissions as needed)
    lambdaRole.addManagedPolicy(iam.ManagedPolicy.fromAwsManagedPolicyName('service-role/AWSLambdaBasicExecutionRole'));

    // Create a policy statement for creating SNS topics
    const sqsPolicyStatement = new iam.PolicyStatement();
    sqsPolicyStatement.addActions('sqs:CreateQueue', 'sqs:SendMessage'); // Add additional permissions as needed
    sqsPolicyStatement.addAllResources(); // Grant access to all SNS resources, adjust as needed

    // Attach the policy statement to the IAM role
    lambdaRole.addToPolicy(sqsPolicyStatement);
  

    const addRecipeWorker = new Function(this, 'addRecipeWorker', {
      description: "Add recipes worker",
      code: Code.fromAsset('lib/lambdas/addRecipeWorker/target/x86_64-unknown-linux-musl/release/lambda'),
      runtime: Runtime.PROVIDED_AL2,
      handler: 'not.required',
      timeout: Duration.minutes(5),
      environment: {
        RUST_BACKTRACE: '1',
        TABLE_NAME: 'Recipes',
        OPEN_AI_API_KEY: openAiApiKey,
        BUCKET_NAME: s3Bucket.bucketName,
        CLOUD_CONVERT_API_KEY: cloudConvertApiKey
      },
      logRetention: RetentionDays.ONE_WEEK,
      role: lambdaRole
    });

    s3Bucket.grantWrite(addRecipeWorker);
    s3Bucket.addToResourcePolicy(
      new iam.PolicyStatement({
        actions: ['s3:PutBucketPolicy'],
        resources: [s3Bucket.bucketArn],
        principals: [new iam.ServicePrincipal('lambda.amazonaws.com')],
      })
    );
    // Create an SNS topic and subscribe the addRecipeWorker Lambda function
    const recipeTopic = new sns.Topic(this, 'AddRecipeTopic');
    recipeTopic.addSubscription(new LambdaSubscription(addRecipeWorker));

    const addRecipe = new Function(this, 'addRecipe', {
      description: "Add recipes worker",
      code: Code.fromAsset('lib/lambdas/addRecipe/target/x86_64-unknown-linux-musl/release/lambda'),
      runtime: Runtime.PROVIDED_AL2,
      handler: 'not.required',
      timeout: Duration.minutes(5),
      environment: {
        RUST_BACKTRACE: '1',
        TABLE_NAME: 'Recipes',
        SNS_ARN: recipeTopic.topicArn
      },
      logRetention: RetentionDays.ONE_WEEK,
      role: lambdaRole
    });

    recipeTopic.grantPublish(addRecipe);

    // Update a recipe
    const updateRecipe = new Function(this, 'updateRecipe', {
      description: "Update recipes",
      code: Code.fromAsset('lib/lambdas/updateRecipe/target/x86_64-unknown-linux-musl/release/lambda'),
      runtime: Runtime.PROVIDED_AL2,
      handler: 'not.required',
      timeout: Duration.minutes(5),
      environment: {
        RUST_BACKTRACE: '1',
        TABLE_NAME: 'Recipes'
      },
      logRetention: RetentionDays.ONE_WEEK,
      role: lambdaRole
    });

    // Add recipe to user's recipe book
    const collectRecipe = new Function(this, 'collectRecipe', {
      description: "Add user recipe",
      code: Code.fromAsset('lib/lambdas/collectRecipe/target/x86_64-unknown-linux-musl/release/lambda'),
      runtime: Runtime.PROVIDED_AL2,
      handler: 'not.required',
      environment: {
        RUST_BACKTRACE: '1',
        TABLE_NAME: 'RecipeBooks',
      },
      logRetention: RetentionDays.ONE_WEEK
    });

    // Get a user's recipe book
    const getUserRecipes = new Function(this, 'getUserRecipes', {
      description: "Get user's recipe book",
      code: Code.fromAsset('lib/lambdas/getUserRecipes/target/x86_64-unknown-linux-musl/release/lambda'),
      runtime: Runtime.PROVIDED_AL2,
      handler: 'not.required',
      environment: {
        RUST_BACKTRACE: '1',
        USER_TABLE_NAME: 'RecipeBooks',
        RECIPE_TABLE_NAME: 'Recipes',
      },
      logRetention: RetentionDays.ONE_WEEK
    });

    userTable.grantFullAccess(collectRecipe);
    userTable.grantFullAccess(getUserRecipes);

    // Gets all recipes from dynamoDB
    const getRecipes = new Function(this, 'getRecipes', {
      description: "Add recipes",
      code: Code.fromAsset('lib/lambdas/getRecipes/target/x86_64-unknown-linux-musl/release/lambda'),
      runtime: Runtime.PROVIDED_AL2,
      handler: 'not.required',
      environment: {
        RUST_BACKTRACE: '1',
        TABLE_NAME: 'Recipes',
      },
      logRetention: RetentionDays.ONE_WEEK
    });

    // Gets all recipes from dynamoDB
    const tesseract = new Function(this, 'tesseract', {
      description: "Add recipes",
      code: Code.fromAsset('lib/lambdas/tesseract/target/x86_64-unknown-linux-musl/release/lambda'),
      runtime: Runtime.PROVIDED_AL2,
      handler: 'not.required',
      timeout: Duration.minutes(5),
      environment: {
        RUST_BACKTRACE: '1',
        OPEN_AI_API_KEY: openAiApiKey,
      },
      logRetention: RetentionDays.ONE_WEEK
    });


    // Lambda for minting a recipe
    const mintNFT = new Function(this, 'mintRecipe', {
      code: lambda.Code.fromAsset(path.join(__dirname, '/lambdas/mintRecipe')),
      runtime: Runtime.NODEJS_16_X,
      handler: 'handler.handler',
      timeout: Duration.minutes(5),
      environment: {
        NFT_STORAGE_API_KEY: nftStoreApiKey,
        PRIVATE_KEY: privateKey,
      }
    });

    // Grant the lambda functions write and read access
    dynamoTable.grantFullAccess(getRecipes);
    dynamoTable.grantFullAccess(addRecipe);
    dynamoTable.grantFullAccess(addRecipeWorker);   
    dynamoTable.grantReadData(getUserRecipes); 
    dynamoTable.grantFullAccess(updateRecipe);


    // Create an API Gateway resource for each of the CRUD operations
    const api = new RestApi(this, 'Recipe3API', {
      restApiName: 'Recipe3 API',
      defaultCorsPreflightOptions: {
        allowOrigins: Cors.ALL_ORIGINS,
        allowMethods: Cors.ALL_METHODS,
        allowHeaders: Cors.DEFAULT_HEADERS,
      }
    });

    // Integrate lambda functions with an API gateway
    const mintNFTAPI = new LambdaIntegration(mintNFT);
    const getRecipesAPI = new LambdaIntegration(getRecipes);
    const addRecipeAPI = new LambdaIntegration(addRecipe);
    const collectRecipeAPI = new LambdaIntegration(collectRecipe);
    const getUserRecipesAPI = new LambdaIntegration(getUserRecipes);
    const tesseractAPI = new LambdaIntegration(tesseract);
    const updateRecipeAPI = new LambdaIntegration(updateRecipe);

    const mint = api.root.addResource('mint');
    mint.addMethod('POST', mintNFTAPI);

    const books = api.root.addResource('api');
    books.addMethod('POST', addRecipeAPI);
    books.addMethod('GET', getRecipesAPI);

    const collect = api.root.addResource('collect');
    collect.addMethod('POST', collectRecipeAPI);
    collect.addMethod('GET', getUserRecipesAPI);

    const tess = api.root.addResource('tesseract');
    tess.addMethod('POST', tesseractAPI);

    const update = api.root.addResource('update');
    update.addMethod('POST', updateRecipeAPI);
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
