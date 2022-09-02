// minecraft honeypot does honeypot things for minecraft and proxies which is cool
// Copyright (C) 2022 cleonyc

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use azalea_protocol::packets::handshake::ServerboundHandshakePacket;
use azalea_protocol::packets::login::ServerboundLoginPacket;
use azalea_protocol::read::read_packet;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

// pub async fn read_packet<'a, P: ProtocolPacket, R>(
//     stream: &'a mut R,
// ) -> anyhow::Result<(Option<P>, Vec<u8>)>
// where
//     R: AsyncRead + std::marker::Unpin + std::marker::Send + std::marker::Sync,
// {
//     // let start_time = std::time::Instant::now();

//     // println!("decrypting packet ({}ms)", start_time.elapsed().as_millis());
//     // if we were given a cipher, decrypt the packet
//     let buf = frame_splitter(stream).await.unwrap();
//     println!("24");
//     // println!("splitting packet ({}ms)", start_time.elapsed().as_millis());
//     if buf.1.len() == 0 {
//         return Ok((None, buf.1));
//     }
//     let mut orig = buf.1.clone();
//     println!("31");
//     // println!("decoding packet ({}ms)", start_time.elapsed().as_millis());
//     let packet: (Option<P>, Vec<u8>) = packet_decoder(&mut buf.0.as_slice()).await?;
//     println!("34");
//     // println!("decoded packet ({}ms)", start_time.elapsed().as_millis());
//     // orig.push(packet.1);
//     Ok((packet.0, orig))
// }

async fn frame_splitter<R: Sized>(mut stream: &mut R) -> anyhow::Result<(Vec<u8>, Vec<u8>)>
where
    R: AsyncRead + std::marker::Unpin + std::marker::Send,
{
    // Packet Length
    // println!("called");
    let res = match read_varint_async(&mut stream).await {
        Ok(len) => len,
        Err(_) => {
            // println!("err reading varint");
            return Ok((vec![], vec![]));
        }
    };
    // println!("fs varint read: {}", res.0);

    if res.0 > 1024 || res.0 == 0 {
        return Ok((vec![], res.1));
    }
    let mut read = vec![];
    let length = res.0;
    while read.len() < length.try_into().unwrap() {
        let mut buf = [0; 1];
        match stream.read_exact(&mut buf).await {
            Ok(_) => read.push(buf[0]),
            Err(_) => {
                break;
            }
        };
    }

    let mut orig = res.1;
    // println!("74: orig starts {:?}, appending: {:?}", orig, read);

    orig.append(&mut (read.clone()));
    let (valid_packet, _) = safe_check_packet_id(&mut read.as_slice()).await;
    if !valid_packet {
        return Ok((vec![], orig));
    }
    Ok((read, orig))
}

// async fn packet_decoder<P: ProtocolPacket, R: Sized>(
//     mut stream: &mut R,
// ) -> anyhow::Result<(Option<P>, Vec<u8>)>
// where
//     R: AsyncRead + std::marker::Unpin + std::marker::Send + std::io::Read,
// {
//     // Packet ID
//     let packet_id = read_varint_async(&mut stream).await?;
//     if packet_id.0 != 0x00 {
//         return Ok((None, packet_id.1));
//     }
//     let read = P::read(packet_id.0.try_into().unwrap(), stream)?;

//     Ok((Some(read), vec![]))
// }
// fast varints modified from https://github.com/luojia65/mc-varint/blob/master/src/lib.rs#L67
/// Read a single varint from the reader and return the value, along with the number of bytes read
pub async fn read_varint_async(
    reader: &mut (dyn AsyncRead + Unpin + Send),
) -> anyhow::Result<(i32, Vec<u8>)> {
    let mut buffer = [0];
    let mut orig = vec![];
    let mut ans = 0;
    for i in 0..5 {
        if let Ok(n) = reader.read(&mut buffer).await && n > 0 {
            ans |= ((buffer[0] & 0b0111_1111) as i32) << (7 * i);
            orig.push(buffer[0]);
            if buffer[0] & 0b1000_0000 == 0 {
                return Ok((ans, orig));
            }
        };
    }
    Ok((ans, orig))
}
pub async fn safe_check_packet_id(reader: &mut (dyn AsyncRead + Unpin + Send)) -> (bool, Vec<u8>) {
    let packet_id = match read_varint_async(reader).await {
        Ok(packet_id) => packet_id,
        Err(_) => return (false, vec![]),
    };
    if packet_id.0 != 0x00 {
        return (false, packet_id.1);
    };
    return (true, packet_id.1);
}
#[derive(Clone, Debug)]
pub enum PossiblePacket {
    LoginStart { packet: ServerboundLoginPacket },
    Status { packet: ServerboundHandshakePacket },
}
pub async fn try_get_packet<R: Sized, W: Sized>(
    stream: &mut R,
    writer: &mut W,
) -> Option<PossiblePacket>
where
    R: AsyncRead + std::marker::Unpin + std::marker::Send,
    W: AsyncWrite + std::marker::Unpin + std::marker::Send,
{
    let (read_bytes, original_bytes) = frame_splitter(stream).await.unwrap();
    // println!("138: orig from frame_splitter: {:?}", original_bytes);
    // 1024 bytes *should* be the theoretical maximum for login or status packets that we care about
    // might break if it's an actual valid user key, we'll see
    if original_bytes.len() > 20000 || original_bytes.len() == 0 || read_bytes.len() == 0 {
        // println!("bad byte len");
        writer.write_all(&original_bytes).await.unwrap();
        // io::copy(stream, writer).await.unwrap();
        return None;
    }
    if let Ok(packet) = read_packet::<ServerboundHandshakePacket, &[u8]>(
        &mut original_bytes.clone().as_slice(),
        None,
        &mut None,
    )
    .await
    {
        writer.write_all(&original_bytes).await.unwrap();
        // io::copy(stream, writer).await.unwrap();
        // println!("returning! <3");
        return Some(PossiblePacket::Status { packet });
    };
    if let Ok(packet) = read_packet::<ServerboundLoginPacket, &[u8]>(
        &mut adapt_from_1_18(&original_bytes.clone()).as_slice(),
        None,
        &mut None,
    )
    .await
    {
        // println!("\n\n\n\n\n\n\n\n\nreturning! (login) <3");
        writer.write_all(&original_bytes).await.unwrap();
        // io::copy(stream, writer).await.unwrap();
        // println!("returning! L<3");
        return Some(PossiblePacket::LoginStart { packet });
    }

    // println!("returning! :(");
    writer.write_all(&original_bytes).await.unwrap();
    // io::copy(stream, writer).await.unwrap();
    None
}
pub async fn get_all_packets<R: Sized, W: Sized>(
    stream: &mut R,
    writer: &mut W,
) -> Vec<PossiblePacket>
where
    R: AsyncRead + std::marker::Unpin + std::marker::Send,
    W: AsyncWrite + std::marker::Unpin + std::marker::Send,
{
    let mut ret = vec![];
    while let Some(packet) = try_get_packet(stream, writer).await {
        ret.push(packet);
    }
    ret
}

fn adapt_from_1_18(bytes: &Vec<u8>) -> Vec<u8> {
    let mut clone = bytes.clone();
    clone[0] += 2;
    clone.push(0);
    clone.push(0);
    clone
}
mod tests {
    use azalea_protocol::{
        packets::handshake::client_intention_packet::ClientIntentionPacket, write::write_packet,
    };
    use rand::Rng;
    use tokio::io::AsyncWriteExt;

    use crate::packet::{adapt_from_1_18, get_all_packets, try_get_packet};

    #[tokio::test]
    async fn test_adapt_1_18() {
        assert_eq!(
            adapt_from_1_18(&vec![6, 0, 4, 50, 57, 55, 56]),
            vec![8, 0, 4, 50, 57, 55, 56, 0, 0]
        );
    }
    #[tokio::test]
    async fn check_packets() {
        // let hello_packet = ServerboundHelloPacket {
        //     username: "2978".to_string(),
        //     public_key: None,
        //     profile_id: None,
        // }
        // .get();
        let mut random_data: [u8; 2048] = [0; 2048];
        rand::thread_rng().fill(&mut random_data);
        let random_data_orig = random_data.clone();
        {
            let mut buf = vec![];

            write_packet(
                ClientIntentionPacket {
                    protocol_version: 758,
                    hostname: "localhost".to_string(),
                    port: 25565,
                    intention: azalea_protocol::packets::ConnectionProtocol::Login,
                }
                .get(),
                &mut buf,
                None,
                &mut None,
            )
            .await
            .unwrap();
            // old 1.18.2 hello packet
            buf.write_all(&vec![6, 0, 4, 50, 57, 55, 56]).await.unwrap();

            // write_packet(hello_packet, &mut buf, None, &mut None)
            //     .await
            //     .unwrap();
            let mut orig_data = vec![];
            let packets = get_all_packets(&mut buf.as_slice(), &mut orig_data).await;
            assert_eq!(packets.len(), 2);
            println!("packets: {:?}", packets);
            assert_eq!(buf, orig_data);
        }
        {
            let mut modded_rand_data: &[u8] = &random_data;
            let mut orig_data = vec![];
            let packet = try_get_packet(&mut modded_rand_data, &mut orig_data).await;
            assert!(packet.is_none());
            assert_eq!(orig_data, random_data_orig);
        }
    }
}

// fn combine(first: &Vec<u8>, second: &Vec<u8>) -> Vec<u8> {
//     // println!("combining {:?} with {:?}", first, second);
//     let mut new = vec![];
//     new.append(&mut first.clone());
//     new.append(&mut second.clone());
//     new
// }
