import Data.Maybe
import Data.List
import System.IO
import System.Random
import GHC.Num


data Size = Size Int Int deriving Show;

type Textures = [String];

type Tiles = [Integer];

data Scene = Scene Size Textures Tiles deriving Show;

stringToSize :: String -> Size
stringToSize s = let (width, height) = splitAt (fromJust (findIndex (== 'x') s)) s
    in Size (read width) (read (tail height));

stringToTiles :: String -> [Integer]
stringToTiles = (map read) . words;

sceneRest :: Size -> [String] -> Scene
sceneRest size s = let (textures, tiles) = ((init s), (last s))
    in Scene size textures (stringToTiles tiles);

stringToScene :: String -> Scene
stringToScene s = let sceneLines = lines s
    in sceneRest (stringToSize (head sceneLines)) (tail sceneLines);

loadScene :: FilePath -> IO Scene
loadScene = (fmap stringToScene) . readFile';

sizeToString :: Size -> String
sizeToString (Size width height) = (show width) ++ ('x' : (show height));

tilesToString :: [Integer] -> String
tilesToString = (foldr1 (\x acc -> x ++ (' ' : acc))) . (map show);

sceneToString :: Scene -> String
sceneToString (Scene size textures tiles) = unlines (((sizeToString size) : textures) ++ [tilesToString tiles]);

saveScene :: FilePath -> Scene -> IO ()
saveScene path = (writeFile path) . sceneToString

modifyScene :: (Scene -> Scene) -> FilePath -> IO ()
modifyScene f path = (fmap f (loadScene path)) >>= (saveScene path);

sceneWith :: Size -> Textures -> (Int -> Tiles) -> Scene
sceneWith (Size width height) textures f = Scene (Size width height) textures (f (width * height));

filledScene :: Size -> Textures -> Integer -> Scene
filledScene size textures value = sceneWith size textures (\amount -> take amount (repeat value));

randomFilledScene :: Size -> Textures -> [Integer] -> IO Scene
randomFilledScene size textures choices = let indicesA = fmap (\gen -> randomRs (0, (integerFromInt (length textures - 1))) gen) getStdGen
    in fmap (\indices -> sceneWith size textures (\amount -> take amount indices)) indicesA;

main :: IO ()
main = return ()
