use axum::extract::State;
use axum::http::StatusCode;
use axum::{
    routing::{get, post},
    Json, Router,
};
use base58::{FromBase58, ToBase58};
use base64::{engine::general_purpose, Engine};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer},
    system_instruction,
};
use spl_token::instruction as token_instruction;
use spl_token::ID as TOKEN_PROGRAM_ID;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
struct AppState;

#[derive(Serialize)]
struct SuccessResponse<T> {
    success: bool,
    data: T,
}

#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
    error: String,
}

fn error(msg: &str) -> Json<ErrorResponse> {
    Json(ErrorResponse {
        success: false,
        error: msg.to_string(),
    })
}

async fn hello_world() -> &'static str {
    "Hello, world!"
}

// 1. Generate Keypair
async fn generate_keypair() -> Json<impl Serialize> {
    let keypair = Keypair::new();
    let pubkey = keypair.pubkey().to_string();
    let secret = keypair.to_bytes().to_base58();

    Json(SuccessResponse {
        success: true,
        data: serde_json::json!({ "pubkey": pubkey, "secret": secret }),
    })
}

// 2. Create Token
#[derive(Deserialize)]
struct CreateTokenReq {
    mint_authority: String,
    mint: String,
    decimals: u8,
}

async fn create_token(
    Json(req): Json<CreateTokenReq>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, Json<ErrorResponse>> {
    let mint = Pubkey::from_str(&req.mint).map_err(|_| error("Invalid mint pubkey"))?;
    let mint_authority = Pubkey::from_str(&req.mint_authority)
        .map_err(|_| error("Invalid mint authority pubkey"))?;

    let ix = token_instruction::initialize_mint(
        &TOKEN_PROGRAM_ID,
        &mint,
        &mint_authority,
        None,
        req.decimals,
    )
    .map_err(|_| error("Failed to create initialize_mint instruction"))?;

    Ok(Json(SuccessResponse {
        success: true,
        data: serde_json::json!({
            "program_id": ix.program_id.to_string(),
            "accounts": ix.accounts,
            "instruction_data": general_purpose::STANDARD.encode(ix.data),
        }),
    }))
}

// 3. Mint Token
#[derive(Deserialize)]
struct MintTokenReq {
    mint: String,
    destination: String,
    authority: String,
    amount: u64,
}

async fn mint_token(
    Json(req): Json<MintTokenReq>,
) -> Result<Json<SuccessResponse<Value>>, Json<ErrorResponse>> {
    let mint = Pubkey::from_str(&req.mint).map_err(|_| error("Invalid mint pubkey"))?;
    let dest =
        Pubkey::from_str(&req.destination).map_err(|_| error("Invalid destination pubkey"))?;
    let auth = Pubkey::from_str(&req.authority).map_err(|_| error("Invalid authority pubkey"))?;

    let ix = token_instruction::mint_to(&TOKEN_PROGRAM_ID, &mint, &dest, &auth, &[], req.amount)
        .map_err(|_| error("Failed to create mint_to instruction"))?;

    Ok(Json(SuccessResponse {
        success: true,
        data: serde_json::json!({
            "program_id": ix.program_id.to_string(),
            "accounts": ix.accounts,
            "instruction_data": general_purpose::STANDARD.encode(ix.data),
        }),
    }))
}

// 4. Sign Message
#[derive(Deserialize)]
struct SignMessageReq {
    message: String,
    secret: String,
}

async fn sign_message(
    Json(req): Json<SignMessageReq>,
) -> Result<Json<SuccessResponse<Value>>, Json<ErrorResponse>> {
    let secret_bytes = req
        .secret
        .from_base58()
        .map_err(|_| error("Invalid secret format"))?;
    let keypair = Keypair::from_bytes(&secret_bytes).map_err(|_| error("Invalid secret key"))?;
    let signature = keypair.sign_message(req.message.as_bytes());

    Ok(Json(SuccessResponse {
        success: true,
        data: serde_json::json!({
            "signature": general_purpose::STANDARD.encode(signature.as_ref()),
            "public_key": keypair.pubkey().to_string(),
            "message": req.message
        }),
    }))
}

// 5. Verify Message
#[derive(Deserialize)]
struct VerifyMessageReq {
    message: String,
    signature: String,
    pubkey: String,
}

async fn verify_message(
    Json(req): Json<VerifyMessageReq>,
) -> Result<Json<SuccessResponse<Value>>, Json<ErrorResponse>> {
    let pubkey = Pubkey::from_str(&req.pubkey).map_err(|_| error("Invalid pubkey format"))?;
    let signature_bytes = general_purpose::STANDARD
        .decode(&req.signature)
        .map_err(|_| error("Invalid base64 signature"))?;
    let signature =
        Signature::try_from(&signature_bytes[..]).map_err(|_| error("Invalid signature format"))?;
    let valid = signature.verify(pubkey.as_ref(), req.message.as_bytes());

    // let valid = Signature::verify(&signature, pubkey.as_ref(), req.message.as_bytes()).is_ok();


    Ok(Json(SuccessResponse {
        success: true,
        data: serde_json::json!({
            "valid": valid,
            "message": req.message,
            "pubkey": req.pubkey
        }),
    }))
}

// 6. Send SOL
#[derive(Deserialize)]
struct SendSolReq {
    from: String,
    to: String,
    lamports: u64,
}

async fn send_sol(
    Json(req): Json<SendSolReq>,
) -> Result<Json<SuccessResponse<Value>>, Json<ErrorResponse>> {
    let from = Pubkey::from_str(&req.from).map_err(|_| error("Invalid sender pubkey"))?;
    let to = Pubkey::from_str(&req.to).map_err(|_| error("Invalid recipient pubkey"))?;

    let ix = system_instruction::transfer(&from, &to, req.lamports);

    Ok(Json(SuccessResponse {
        success: true,
        data: serde_json::json!({
            "program_id": ix.program_id.to_string(),
            "accounts": ix.accounts.iter().map(|a| a.pubkey.to_string()).collect::<Vec<_>>(),
            "instruction_data": general_purpose::STANDARD.encode(ix.data),
        }),
    }))
}

// 7. Send Token
#[derive(Deserialize)]
struct SendTokenReq {
    destination: String,
    mint: String,
    owner: String,
    amount: u64,
}

async fn send_token(
    Json(req): Json<SendTokenReq>,
) -> Result<Json<SuccessResponse<Value>>, Json<ErrorResponse>> {
    let dest =
        Pubkey::from_str(&req.destination).map_err(|_| error("Invalid destination pubkey"))?;
    let mint = Pubkey::from_str(&req.mint).map_err(|_| error("Invalid mint pubkey"))?;
    let owner = Pubkey::from_str(&req.owner).map_err(|_| error("Invalid owner pubkey"))?;

    let ix = token_instruction::transfer(&TOKEN_PROGRAM_ID, &mint, &dest, &owner, &[], req.amount)
        .map_err(|_| error("Failed to create transfer instruction"))?;

    Ok(Json(SuccessResponse {
        success: true,
        data: serde_json::json!({
            "program_id": ix.program_id.to_string(),
            "accounts": ix.accounts,
            "instruction_data": general_purpose::STANDARD.encode(ix.data),
        }),
    }))
}

#[shuttle_runtime::main]
async fn main() -> shuttle_axum::ShuttleAxum {
    let router = Router::new()
        .route("/", get(hello_world))
        .route("/keypair", post(generate_keypair))
        .route("/token/create", post(create_token))
        .route("/token/mint", post(mint_token))
        .route("/message/sign", post(sign_message))
        .route("/message/verify", post(verify_message))
        .route("/send/sol", post(send_sol))
        .route("/send/token", post(send_token));

    Ok(router.into())
}


// use axum::{routing::post, Json, Router};
// use axum::http::StatusCode;
// use base58::{FromBase58, ToBase58};
// use base64::{engine::general_purpose, Engine};
// use serde::{Deserialize, Serialize};
// use serde_json::Value;
// use solana_sdk::{
//     instruction::{AccountMeta, Instruction},
//     pubkey::Pubkey,
//     signature::{Keypair, Signature, Signer},
//     system_instruction,
//     system_program,
//     sysvar::rent,
// };
// use spl_associated_token_account::get_associated_token_address;
// use spl_token::instruction as token_instruction;
// use spl_token::ID as TOKEN_PROGRAM_ID;
// use std::str::FromStr;

// #[derive(Serialize)]
// struct SuccessResponse<T> {
//     success: bool,
//     data: T,
// }

// #[derive(Serialize)]
// struct ErrorResponse {
//     success: bool,
//     error: String,
// }

// fn error(msg: &str) -> Json<ErrorResponse> {
//     Json(ErrorResponse {
//         success: false,
//         error: msg.to_string(),
//     })
// }

// async fn hello_world() -> &'static str {
//     "Hello, world!"
// }

// // 1. Generate Keypair
// async fn generate_keypair() -> Json<SuccessResponse<Value>> {
//     let keypair = Keypair::new();
//     let pubkey = keypair.pubkey().to_string();
//     let secret = keypair.to_bytes().to_base58();

//     Json(SuccessResponse {
//         success: true,
//         data: serde_json::json!({ "pubkey": pubkey, "secret": secret }),
//     })
// }

// // 2. Create Token
// #[derive(Deserialize)]
// struct CreateTokenReq {
//     mint_authority: String,
//     mint: String,
//     decimals: u8,
// }

// #[derive(Serialize)]
// struct Account {
//     pubkey: String,
//     is_signer: bool,
//     is_writable: bool,
// }

// async fn create_token(
//     Json(req): Json<CreateTokenReq>,
// ) -> Result<Json<SuccessResponse<Value>>, Json<ErrorResponse>> {
//     let mint = Pubkey::from_str(&req.mint).map_err(|_| error("Invalid mint pubkey"))?;
//     let mint_authority = Pubkey::from_str(&req.mint_authority)
//         .map_err(|_| error("Invalid mint authority pubkey"))?;

//     let ix = token_instruction::initialize_mint(
//         &TOKEN_PROGRAM_ID,
//         &mint,
//         &mint_authority,
//         None,
//         req.decimals,
//     )
//     .map_err(|_| error("Failed to create initialize_mint instruction"))?;

//     let accounts = vec![
//         Account {
//             pubkey: mint.to_string(),
//             is_signer: false,
//             is_writable: true,
//         },
//         Account {
//             pubkey: rent::ID.to_string(),
//             is_signer: false,
//             is_writable: false,
//         },
//     ];

//     Ok(Json(SuccessResponse {
//         success: true,
//         data: serde_json::json!({
//             "program_id": ix.program_id.to_string(),
//             "accounts": accounts,
//             "instruction_data": general_purpose::STANDARD.encode(ix.data),
//         }),
//     }))
// }

// // 3. Mint Token
// #[derive(Deserialize)]
// struct MintTokenReq {
//     mint: String,
//     destination: String,
//     authority: String,
//     amount: u64,
// }

// async fn mint_token(
//     Json(req): Json<MintTokenReq>,
// ) -> Result<Json<SuccessResponse<Value>>, Json<ErrorResponse>> {
//     let mint = Pubkey::from_str(&req.mint).map_err(|_| error("Invalid mint pubkey"))?;
//     let destination = Pubkey::from_str(&req.destination)
//         .map_err(|_| error("Invalid destination pubkey"))?;
//     let authority = Pubkey::from_str(&req.authority)
//         .map_err(|_| error("Invalid authority pubkey"))?;

//     let ix = token_instruction::mint_to(&TOKEN_PROGRAM_ID, &mint, &destination, &authority, &[], req.amount)
//         .map_err(|_| error("Failed to create mint_to instruction"))?;

//     let accounts = vec![
//         Account {
//             pubkey: mint.to_string(),
//             is_signer: false,
//             is_writable: true,
//         },
//         Account {
//             pubkey: destination.to_string(),
//             is_signer: false,
//             is_writable: true,
//         },
//         Account {
//             pubkey: authority.to_string(),
//             is_signer: true,
//             is_writable: false,
//         },
//     ];

//     Ok(Json(SuccessResponse {
//         success: true,
//         data: serde_json::json!({
//             "program_id": ix.program_id.to_string(),
//             "accounts": accounts,
//             "instruction_data": general_purpose::STANDARD.encode(ix.data),
//         }),
//     }))
// }

// // 4. Sign Message
// #[derive(Deserialize)]
// struct SignMessageReq {
//     message: String,
//     secret: String,
// }

// async fn sign_message(
//     Json(req): Json<SignMessageReq>,
// ) -> Result<Json<SuccessResponse<Value>>, Json<ErrorResponse>> {
//     if req.message.is_empty() || req.secret.is_empty() {
//         return Err(error("Missing required fields"));
//     }
//     let secret_bytes = req
//         .secret
//         .from_base58()
//         .map_err(|_| error("Invalid secret format"))?;
//     let keypair = Keypair::from_bytes(&secret_bytes).map_err(|_| error("Invalid secret key"))?;
//     let signature = keypair.sign_message(req.message.as_bytes());

//     Ok(Json(SuccessResponse {
//         success: true,
//         data: serde_json::json!({
//             "signature": general_purpose::STANDARD.encode(signature.as_ref()),
//             "public_key": keypair.pubkey().to_string(),
//             "message": req.message
//         }),
//     }))
// }

// // 5. Verify Message
// #[derive(Deserialize)]
// struct VerifyMessageReq {
//     message: String,
//     signature: String,
//     pubkey: String,
// }

// async fn verify_message(
//     Json(req): Json<VerifyMessageReq>,
// ) -> Result<Json<SuccessResponse<Value>>, Json<ErrorResponse>> {
//     if req.message.is_empty() || req.signature.is_empty() || req.pubkey.is_empty() {
//         return Err(error("Missing required fields"));
//     }
//     let pubkey = Pubkey::from_str(&req.pubkey).map_err(|_| error("Invalid pubkey format"))?;
//     let signature_bytes = general_purpose::STANDARD
//         .decode(&req.signature)
//         .map_err(|_| error("Invalid base64 signature"))?;
//     let signature = Signature::try_from(&signature_bytes[..])
//         .map_err(|_| error("Invalid signature format"))?;
//     // Corrected verification: Use Signature::verify
//     let valid = signature.verify(pubkey.as_ref(), req.message.as_bytes());

//     Ok(Json(SuccessResponse {
//         success: true,
//         data: serde_json::json!({
//             "valid": valid,
//             "message": req.message,
//             "pubkey": req.pubkey
//         }),
//     }))
// }

// // 6. Send SOL
// #[derive(Deserialize)]
// struct SendSolReq {
//     from: String,
//     to: String,
//     lamports: u64,
// }

// async fn send_sol(
//     Json(req): Json<SendSolReq>,
// ) -> Result<Json<SuccessResponse<Value>>, Json<ErrorResponse>> {
//     if req.lamports == 0 {
//         return Err(error("Invalid lamports amount"));
//     }
//     let from = Pubkey::from_str(&req.from).map_err(|_| error("Invalid sender pubkey"))?;
//     let to = Pubkey::from_str(&req.to).map_err(|_| error("Invalid recipient pubkey"))?;

//     let ix = system_instruction::transfer(&from, &to, req.lamports);

//     let accounts = vec![
//         Account {
//             pubkey: from.to_string(),
//             is_signer: true,
//             is_writable: true,
//         },
//         Account {
//             pubkey: to.to_string(),
//             is_signer: false,
//             is_writable: true,
//         },
//     ];

//     Ok(Json(SuccessResponse {
//         success: true,
//         data: serde_json::json!({
//             "program_id": ix.program_id.to_string(),
//             "accounts": accounts,
//             "instruction_data": general_purpose::STANDARD.encode(ix.data),
//         }),
//     }))
// }

// // 7. Send Token
// #[derive(Deserialize)]
// struct SendTokenReq {
//     destination: String,
//     mint: String,
//     owner: String,
//     amount: u64,
// }

// async fn send_token(
//     Json(req): Json<SendTokenReq>,
// ) -> Result<Json<SuccessResponse<Value>>, Json<ErrorResponse>> {
//     if req.amount == 0 {
//         return Err(error("Invalid amount"));
//     }
//     let mint = Pubkey::from_str(&req.mint).map_err(|_| error("Invalid mint pubkey"))?;
//     let owner = Pubkey::from_str(&req.owner).map_err(|_| error("Invalid owner pubkey"))?;
//     let destination = Pubkey::from_str(&req.destination)
//         .map_err(|_| error("Invalid destination pubkey"))?;

//     // Derive source and destination associated token accounts
//     let source_ata = get_associated_token_address(&owner, &mint);
//     let dest_ata = get_associated_token_address(&destination, &mint);

//     let ix = token_instruction::transfer(
//         &TOKEN_PROGRAM_ID,
//         &source_ata,
//         &dest_ata,
//         &owner,
//         &[],
//         req.amount,
//     )
//     .map_err(|_| error("Failed to create transfer instruction"))?;

//     let accounts = vec![
//         Account {
//             pubkey: source_ata.to_string(),
//             is_signer: false,
//             is_writable: true,
//         },
//         Account {
//             pubkey: dest_ata.to_string(),
//             is_signer: false,
//             is_writable: true,
//         },
//         Account {
//             pubkey: owner.to_string(),
//             is_signer: true,
//             is_writable: false,
//         },
//     ];

//     Ok(Json(SuccessResponse {
//         success: true,
//         data: serde_json::json!({
//             "program_id": ix.program_id.to_string(),
//             "accounts": accounts,
//             "instruction_data": general_purpose::STANDARD.encode(ix.data),
//         }),
//     }))
// }

// #[shuttle_runtime::main]
// async fn main() -> shuttle_axum::ShuttleAxum {
//     let router = Router::new()
//         .route("/", post(hello_world))
//         .route("/keypair", post(generate_keypair))
//         .route("/token/create", post(create_token))
//         .route("/token/mint", post(mint_token))
//         .route("/message/sign", post(sign_message))
//         .route("/message/verify", post(verify_message))
//         .route("/send/sol", post(send_sol))
//         .route("/send/token", post(send_token));

//     Ok(router.into())
// }