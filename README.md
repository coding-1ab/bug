# 지렁이 게임

## 게임 실행 방법

```bash
# client 모듈 실행
cargo run --bin client
```

## 서버 실행 방법

```bash
# server 모듈 실행
cargo run --bin server

# server와의 통신 테스트를 위해 작성한 테스트 코드 실행 방법
# (표준 출력/표준 에러출력 포함)
cargo test --bin server -- --nocapture
```

